use crate::common;
use rand::TryRng;
use std::fs::File;
use std::io::Read;
use std::time::Duration;
use std::{
    collections::HashMap,
    net::{SocketAddr, UdpSocket},
};

struct FileTransfer {
    file: File,
    buf: Vec<u8>,
    pos: usize,
    len: usize,
    reached_eof: bool,
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
    let mut active_downloads: HashMap<(quiche::ConnectionId<'static>, u64), FileTransfer> =
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
                        let mut path_buf = [0; 512];
                        if let Ok((read_len, fin)) = conn.stream_recv(stream_id, &mut path_buf) {
                            if !fin {
                                continue;
                            }
                            let file_path =
                                std::str::from_utf8(&path_buf[..read_len]).unwrap().trim();

                            match File::open(file_path) {
                                Ok(file) => {
                                    // Send Success Prefix (0x01) immediately
                                    conn.stream_send(stream_id, &[0x01], false).ok();

                                    // Register the stateful transfer
                                    active_downloads.insert(
                                        (cid.clone(), stream_id),
                                        FileTransfer {
                                            file,
                                            buf: vec![0; 4096], // 4KB chunks keep RAM usage negligible
                                            pos: 0,
                                            len: 0,
                                            reached_eof: false,
                                        },
                                    );
                                }
                                Err(e) => {
                                    // File missing, send error prefix (0x00) + msg
                                    conn.stream_send(stream_id, &[0x00], false).ok();
                                    let err_msg = format!("File error: {}", e);
                                    conn.stream_send(stream_id, err_msg.as_bytes(), true).ok();
                                }
                            }
                        }
                    }

                    // Step C.2: Pump chunks for existing active transfers
                    active_downloads.retain(|id, transfer| {
                        // If our chunk buffer is empty, pull the next block from disk
                        if transfer.pos == transfer.len && !transfer.reached_eof {
                            transfer.pos = 0;
                            match transfer.file.read(&mut transfer.buf) {
                                Ok(0) => {
                                    transfer.reached_eof = true;
                                    transfer.len = 0;
                                }
                                Ok(n) => transfer.len = n,
                                Err(_) => return false, // Drop on disk read failure
                            }
                        }

                        // If we have data to clear out, try shifting it into the QUIC pipe
                        if transfer.len > transfer.pos || transfer.reached_eof {
                            let remaining_chunk = &transfer.buf[transfer.pos..transfer.len];
                            let is_fin = transfer.reached_eof;

                            match conn.stream_send(id.1, remaining_chunk, is_fin) {
                                Ok(written) => {
                                    transfer.pos += written;
                                    // If we hit EOF and flushed the last byte, terminate tracking
                                    if transfer.reached_eof && transfer.pos == transfer.len {
                                        println!(
                                            "[Server] Stream {} download complete!",
                                            id.1
                                        );
                                        return false;
                                    }
                                }
                                Err(quiche::Error::Done) => {
                                    // The QUIC window is full! Stop sending for this specific stream.
                                    // We keep it in the Map and try again on the next loop tick.
                                }
                                Err(_) => return false, // Connection dropped, clean up memory
                            }
                        }
                        true // Keep processing this file on future ticks
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
