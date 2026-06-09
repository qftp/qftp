use crate::common;
use rand::TryRng;
use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::time::Duration;
use std::{
    collections::HashMap,
    net::{SocketAddr, UdpSocket},
};

enum ActiveOperation {
    Downloading(FileTransfer),
    Uploading(FileReceiver),
}

struct FileTransfer {
    file: File,
    buf: Vec<u8>,
    pos: usize,
    len: usize,
    reached_eof: bool,
}

struct FileReceiver {
    file: File,
    completed: bool,
}

pub fn run() {
    let mut config = common::new_quiche_config(true).expect("Failed to create quiche config");

    let local_addr = SocketAddr::from(([127, 0, 0, 1], common::QFTP_PORT));
    let socket = UdpSocket::bind(local_addr).expect("Failed to bind UDP socket");
    socket
        .set_nonblocking(true)
        .expect("Failed to make socket nonblocking");

    let mut buf = [0; 65535];
    let mut out = [0; 65535];

    // Keep track of active connections keyed by Destination Connection ID (DCID)
    let mut conns: HashMap<quiche::ConnectionId<'static>, quiche::Connection> = HashMap::new();

    // Keep track of active transfers (client downloading file(s) from server)
    let mut active_operations: HashMap<(quiche::ConnectionId<'static>, u64), ActiveOperation> =
        HashMap::new();

    println!(
        "qftp server listening on 127.0.0.1:{}...",
        common::QFTP_PORT
    );

    loop {
        // Step A: Read incoming UDP datagrams
        match socket.recv_from(&mut buf) {
            Ok((len, src)) => {
                let pkt_buf = &mut buf[..len];

                // Parse the QUIC packet header
                let header = match quiche::Header::from_slice(pkt_buf, quiche::MAX_CONN_ID_LEN) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("Failed to parse QUIC header: {:?}", e);
                        continue;
                    }
                };

                let dcid = header.dcid.clone().into_owned();

                // Step B: Find existing connection state or negotiate a new connection
                let (conn, cid) = if !conns.contains_key(&dcid) {
                    if header.ty != quiche::Type::Initial {
                        eprintln!(
                            "Packet dropped: Not an Initial packet, and no connection tracking exists."
                        );
                        continue;
                    }

                    // Generate a unique Server Connection ID (SCID)
                    let mut scid_bytes = vec![0u8; quiche::MAX_CONN_ID_LEN];
                    rand::rng()
                        .try_fill_bytes(&mut scid_bytes)
                        .expect("Failed to generate connection ID");
                    let scid = quiche::ConnectionId::from_vec(scid_bytes);

                    // Accept handshake and allocate structural connection resources
                    let conn = quiche::accept(&scid, None, local_addr, src, &mut config)
                        .expect("Failed to accept quiche connection");

                    conns.insert(scid.clone(), conn);
                    (conns.get_mut(&scid).unwrap(), scid)
                } else {
                    (conns.get_mut(&dcid).unwrap(), dcid)
                };

                // Step C: Feed raw wire bytes directly into the parsed connection engine
                let recv_info = quiche::RecvInfo {
                    from: src,
                    to: local_addr,
                };

                if let Err(e) = conn.recv(pkt_buf, recv_info) {
                    eprintln!("Connection read error: {:?}", e);
                    continue;
                }

                if conn.is_established() {
                    // Step C.1: Check for NEW file requests from clients
                    for stream_id in conn.readable() {
                        let mut stream_buf = [0; 8192];

                        // Only parse if we aren't already tracking this connection's stream
                        if let Some(operation) =
                            active_operations.get_mut(&(cid.clone(), stream_id))
                        {
                            match operation {
                                ActiveOperation::Uploading(receiver) => {
                                    match conn.stream_recv(stream_id, &mut stream_buf) {
                                        Ok((read_len, fin)) => {
                                            if read_len > 0 {
                                                receiver
                                                    .file
                                                    .write_all(&stream_buf[..read_len])
                                                    .unwrap();
                                            }
                                            if fin {
                                                receiver.completed = true;
                                                println!(
                                                    "[Server] Conn {:?} Stream {} upload completely written to disk.",
                                                    cid, stream_id
                                                );
                                            }
                                        }
                                        Err(quiche::Error::Done) => {}
                                        Err(_) => receiver.completed = true, // Force clean up on stream errors
                                    }
                                }
                                _ => {} // Downloads don't expect incoming application payload bytes
                            }
                        } else {
                            if let Ok((read_len, fin)) =
                                conn.stream_recv(stream_id, &mut stream_buf)
                            {
                                if read_len > 1 {
                                    let command_byte = stream_buf[0];
                                    let payload = &stream_buf[1..read_len];

                                    match command_byte {
                                        common::CMD_DOWNLOAD => {
                                            let file_path =
                                                std::str::from_utf8(payload).unwrap_or("").trim();
                                            println!(
                                                "[Server] Conn {:?} Stream {} requested DOWNLOAD of: {}",
                                                cid, stream_id, file_path
                                            );

                                            match File::open(file_path) {
                                                Ok(file) => {
                                                    // Acknowledge download start
                                                    conn.stream_send(
                                                        stream_id,
                                                        &[common::RES_DOWNLOAD_START],
                                                        false,
                                                    )
                                                    .ok();

                                                    active_operations.insert(
                                                        (cid.clone(), stream_id),
                                                        ActiveOperation::Downloading(
                                                            FileTransfer {
                                                                file,
                                                                buf: vec![0; 4096],
                                                                pos: 0,
                                                                len: 0,
                                                                reached_eof: false,
                                                            },
                                                        ),
                                                    );
                                                }
                                                Err(e) => {
                                                    conn.stream_send(
                                                        stream_id,
                                                        &[common::RES_ERROR],
                                                        false,
                                                    )
                                                    .ok();
                                                    let err_msg = format!("File missing: {}", e);
                                                    conn.stream_send(
                                                        stream_id,
                                                        err_msg.as_bytes(),
                                                        true,
                                                    )
                                                    .ok();
                                                }
                                            }
                                        }
                                        common::CMD_UPLOAD => {
                                            let save_path =
                                                std::str::from_utf8(payload).unwrap_or("").trim();
                                            println!(
                                                "[Server] Client requesting UPLOAD allocation at: {}",
                                                save_path
                                            );

                                            match std::fs::OpenOptions::new()
                                                .create(true)
                                                .write(true)
                                                .truncate(true)
                                                .open(save_path)
                                            {
                                                Ok(file) => {
                                                    println!("[Server] Sending upload start");
                                                    conn.stream_send(
                                                        stream_id,
                                                        &[common::RES_UPLOAD_START],
                                                        false,
                                                    )
                                                    .ok();

                                                    active_operations.insert(
                                                        (cid.clone(), stream_id),
                                                        ActiveOperation::Uploading(FileReceiver {
                                                            file,
                                                            completed: fin, // If the file initialization was immediately fin-marked
                                                        }),
                                                    );
                                                }
                                                Err(e) => {
                                                    conn.stream_send(
                                                        stream_id,
                                                        &[common::RES_ERROR],
                                                        false,
                                                    )
                                                    .ok();
                                                    let err_msg = format!(
                                                        "Failed to create destination file: {}",
                                                        e
                                                    );
                                                    conn.stream_send(
                                                        stream_id,
                                                        err_msg.as_bytes(),
                                                        true,
                                                    )
                                                    .ok();
                                                }
                                            }
                                        }
                                        _ => {
                                            eprintln!(
                                                "[Server] Unknown command prefix received: {}",
                                                command_byte
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Step C.2: Pump chunks for existing active transfers
                    active_operations.retain(|(c_key, s_id), operation| {
                        if let Some(current_conn) = conns.get_mut(c_key) {
                            match operation {
                                ActiveOperation::Downloading(transfer) => {
                                    // Populate buffer from disk if empty
                                    if transfer.pos == transfer.len && !transfer.reached_eof {
                                        transfer.pos = 0;
                                        match transfer.file.read(&mut transfer.buf) {
                                            Ok(0) => {
                                                transfer.reached_eof = true;
                                                transfer.len = 0;
                                            }
                                            Ok(n) => transfer.len = n,
                                            Err(_) => return false,
                                        }
                                    }

                                    // Push chunk to the specific connection stream
                                    if transfer.len > transfer.pos || transfer.reached_eof {
                                        let remaining = &transfer.buf[transfer.pos..transfer.len];
                                        match current_conn.stream_send(
                                            *s_id,
                                            remaining,
                                            transfer.reached_eof,
                                        ) {
                                            Ok(written) => {
                                                transfer.pos += written;
                                                if transfer.reached_eof
                                                    && transfer.pos == transfer.len
                                                {
                                                    println!(
                                                        "[Server] Stream {} download complete.",
                                                        s_id
                                                    );
                                                    return false; // Remove operation
                                                }
                                            }
                                            Err(quiche::Error::Done) => {} // Window full, hold tight until next tick
                                            Err(_) => return false,        // Stream broken
                                        }
                                    }
                                }
                                ActiveOperation::Uploading(receiver) => {
                                    if receiver.completed {
                                        return false;
                                    }
                                }
                            }
                            true
                        } else {
                            false // Connection dead
                        }
                    });
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // No raw packet on the socket wire, proceed smoothly to packet drain/timeouts
            }
            Err(e) => {
                eprintln!("Socket read system error: {:?}", e);
                break;
            }
        }

        // Step E: Drain and send pending network packets out to the network interface
        for conn in conns.values_mut() {
            while let Ok((write_len, send_info)) = conn.send(&mut out) {
                if let Err(e) = socket.send_to(&out[..write_len], send_info.to) {
                    eprintln!("Socket writing system error: {:?}", e);
                }
            }
        }

        conns.retain(|_, conn| !conn.is_closed());

        std::thread::sleep(Duration::from_millis(2));
    }
}
