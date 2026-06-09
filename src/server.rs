use crate::common;
use std::io::Write;
use std::time::Duration;
use std::{
    collections::HashMap,
    net::{SocketAddr, UdpSocket},
};
use rand::TryRng;

pub fn run() {
    let mut config = common::new_quiche_config(true).expect("Failed to create quiche config");

    let local_addr = SocketAddr::from(([127, 0, 0, 1], common::QFTP_PORT));
    let socket = UdpSocket::bind(local_addr)
        .expect("Failed to bind UDP socket");
    socket
        .set_nonblocking(true)
        .expect("Failed to make socket nonblocking");

    let mut buf = [0; 65535];
    let mut out = [0; 65535];

    // Keep track of active connections keyed by Destination Connection ID (DCID)
    let mut conns: HashMap<quiche::ConnectionId<'static>, quiche::Connection> = HashMap::new();

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
                let conn = if !conns.contains_key(&dcid) {
                    if header.ty != quiche::Type::Initial {
                        eprintln!(
                            "Packet dropped: Not an Initial packet, and no connection tracking exists."
                        );
                        continue;
                    }

                    // Generate a unique Server Connection ID (SCID)
                    let mut scid_bytes = vec![0u8; quiche::MAX_CONN_ID_LEN];
                    rand::rng().try_fill_bytes(&mut scid_bytes).expect("Failed to generate connection ID");
                    let scid = quiche::ConnectionId::from_vec(scid_bytes);

                    // Accept handshake and allocate structural connection resources
                    let conn = quiche::accept(
                        &scid,
                        None,
                        local_addr,
                        src,
                        &mut config,
                    )
                    .expect("Failed to accept quiche connection");

                    conns.insert(scid.clone(), conn);
                    conns.get_mut(&scid).unwrap()
                } else {
                    conns.get_mut(&dcid).unwrap()
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

                // Step D: Process readable streams if the cryptographic handshake has succeeded
                if conn.is_established() {
                    for stream_id in conn.readable() {
                        let mut stream_buf = [0; 2048];

                        while let Ok((read_len, fin)) = conn.stream_recv(stream_id, &mut stream_buf)
                        {
                            if let Ok(text) = str::from_utf8(&stream_buf[..read_len]) {
                                print!("{}", text);
                                std::io::stdout().flush().unwrap();
                            }
                        }
                    }
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
