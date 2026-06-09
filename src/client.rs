use crate::common::{self, QftpCommand};
use io::Write;
use rand::TryRng;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io;
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

pub fn run() {
    println!("qftp client - type ? for help");
    let mut client = Client::new();

    loop {
        // Prompt
        print!("> ");
        io::stdout().flush().expect("Failed to flush stdout");

        // Get input
        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .expect("Failed to read stdin");

        // Tokenize (support quoted file names with spaces)
        let mut tokens: Vec<String> = vec![];
        let mut token = String::new();
        let mut in_quotes = false;
        for ch in input.trim().chars() {
            match ch {
                ' ' if !in_quotes => {
                    tokens.push(token);
                    token = String::new()
                }
                '"' => in_quotes = !in_quotes,
                _ => token.push(ch),
            }
        }
        if !token.is_empty() {
            tokens.push(token);
        }

        if tokens.is_empty() || tokens[0].is_empty() {
            continue;
        }

        let args = &tokens[1..];

        // Run command
        match tokens[0].as_str() {
            "open" => client.open(args),
            "get" => client.get(args),
            "mget" => client.mget(args),
            "put" => client.put(args),
            "?" => print_help(),
            "q" => break,
            cmd => {
                eprintln!("Unknown command: {cmd}");
            }
        }
    }
}

fn print_help() {
    println!(
        "open <url> - connect to a qftp server
get <file> - download a file
mget <file1> ... <fileX> - download multiple files in parallel
put <file> - upload a file
q - quit
? - print this information"
    )
}

struct ClientSink {
    file_writer: File,
    is_initialized: bool,
}

struct Client {
    sender: Option<mpsc::Sender<QftpCommand>>,
}

impl Client {
    fn new() -> Client {
        return Client { sender: None };
    }

    /// Open a new connection to a qftp server.
    fn open(&mut self, args: &[String]) {
        if args.len() != 1 {
            eprintln!("Usage: open <url>");
            return;
        }

        let url = args[0].as_str();
        let Ok(dest_ip) = url.parse::<IpAddr>() else {
            eprint!("Failed to parse url");
            return;
        };
        let server_addr = SocketAddr::new(dest_ip, common::QFTP_PORT);

        let (tx, rx) = mpsc::channel::<QftpCommand>();

        let mut active_sinks: HashMap<u64, (File, bool)> = HashMap::new();

        thread::spawn(move || {
            let mut config =
                common::new_quiche_config(false).expect("Failed to create quiche config");

            let socket = UdpSocket::bind("127.0.0.1:0").expect("Failed to bind UDP socket");
            socket
                .set_nonblocking(true)
                .expect("Failed to make socket nonblocking");
            let local_addr = socket
                .local_addr()
                .expect("Failed to get UDP socket local_addr");

            let mut scid_bytes = vec![0u8; quiche::MAX_CONN_ID_LEN];
            rand::rng()
                .try_fill_bytes(&mut scid_bytes)
                .expect("Failed to generate connection ID");
            let scid = quiche::ConnectionId::from_vec(scid_bytes);

            let mut conn = quiche::connect(
                Some("localhost"),
                &scid,
                local_addr,
                server_addr,
                &mut config,
            )
            .expect("Failed to create quiche connection");

            let mut buf = [0; 65535];
            let mut out = [0; 65535];

            let mut next_stream_id: u64 = 0;
            let mut client_sinks: HashMap<u64, ClientSink> = HashMap::new();

            loop {
                // Read MPSC channel
                if conn.is_established() {
                    while let Ok(cmd) = rx.try_recv() {
                        match cmd {
                            QftpCommand::Download {
                                remote_path,
                                local_save_path,
                            } => {
                                let stream_id = next_stream_id;
                                next_stream_id += 4; // Reserve next client-initiated bidi stream ID

                                // Open target write space ahead of network initialization
                                if let Ok(file) = OpenOptions::new()
                                    .create(true)
                                    .append(true)
                                    .open(&local_save_path)
                                {
                                    client_sinks.insert(
                                        stream_id,
                                        ClientSink {
                                            file_writer: file,
                                            is_initialized: false,
                                        },
                                    );

                                    // Construct our Framed payload: [CMD_DOWNLOAD] + [Path String]
                                    let mut payload = vec![common::CMD_DOWNLOAD];
                                    payload.extend_from_slice(remote_path.as_bytes());

                                    // Blast command out to server
                                    conn.stream_send(stream_id, &payload, true).unwrap();
                                    println!(
                                        "[Client] Fired concurrent download request for {} on Stream {}",
                                        remote_path, stream_id
                                    );
                                }
                            }
                            QftpCommand::Upload {
                                local_path,
                                remote_save_path,
                            } => {
                                // Prepared structurally for you:
                                // 1. Allocate stream_id += 4
                                // 2. Open local file handle
                                // 3. Send payload: vec![CMD_UPLOAD] + remote_save_path bytes (fin = false)
                                // 4. Store handle in an outbound stream pump map
                            }
                        }
                    }
                }

                // Send UDP data
                loop {
                    match conn.send(&mut out) {
                        Ok((write_len, send_info)) => {
                            if let Err(e) = socket.send_to(&out[..write_len], send_info.to) {
                                eprintln!("[QUIC Background] Socket write failure: {:?}", e);
                            }
                        }
                        Err(quiche::Error::Done) => break,
                        Err(e) => {
                            eprintln!("[QUIC Background] QUIC engine compile error: {:?}", e);
                            break;
                        }
                    }
                }

                if conn.is_closed() {
                    println!("[QUIC Background] Connection closed. Exiting thread.");
                    break;
                }

                // Receive data
                match socket.recv_from(&mut buf) {
                    Ok((len, src)) => {
                        let recv_info = quiche::RecvInfo {
                            from: src,
                            to: socket.local_addr().unwrap(),
                        };

                        if let Err(e) = conn.recv(&mut buf[..len], recv_info) {
                            eprintln!("[QUIC Background] Engine read parse error: {:?}", e);
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(e) => {
                        eprintln!("[QUIC Background] System socket reading error: {:?}", e);
                        break;
                    }
                }

                // Process received streams
                if conn.is_established() {
                    for stream_id in conn.readable() {
                        let mut chunk_buf = [0; 8192];

                        if let Ok((read_len, fin)) = conn.stream_recv(stream_id, &mut chunk_buf) {
                            if read_len > 0 {
                                if let Some(sink) = client_sinks.get_mut(&stream_id) {
                                    let mut data_start = 0;

                                    // First packet chunk contains our server status flag validation header
                                    if !sink.is_initialized {
                                        let status_flag = chunk_buf[0];
                                        data_start = 1;
                                        sink.is_initialized = true;

                                        if status_flag == common::RES_ERROR {
                                            let err_msg =
                                                std::str::from_utf8(&chunk_buf[1..read_len])
                                                    .unwrap_or("Server Error");
                                            eprintln!(
                                                "[Client] Stream {} download failed: {}",
                                                stream_id, err_msg
                                            );
                                            client_sinks.remove(&stream_id);
                                            continue;
                                        }
                                        println!(
                                            "[Client] Stream {} download verified active by server.",
                                            stream_id
                                        );
                                    }

                                    // Append chunk straight to its independent disk tracking descriptor
                                    if read_len > data_start {
                                        sink.file_writer
                                            .write_all(&chunk_buf[data_start..read_len])
                                            .unwrap();
                                    }
                                }
                            }

                            if fin {
                                println!(
                                    "[Client] Stream {} finished pipeline execution.",
                                    stream_id
                                );
                                client_sinks.remove(&stream_id);
                            }
                        }
                    }
                }

                thread::sleep(Duration::from_millis(2));
            }
        });

        self.sender = Some(tx);
    }

    /// Download a single file from the active server connection.
    fn get(&mut self, args: &[String]) {
        if args.len() != 1 {
            eprintln!("Usage: get <file>");
            return;
        }

        let Some(ref mut sender) = self.sender else {
            eprintln!("Start a connection with open <url> before calling get");
            return;
        };

        let file_path = args[0].clone();

        println!("Downloading file {file_path}...");
        match sender.send(QftpCommand::Download {
            remote_path: file_path,
            local_save_path: "result.dat".to_string(),
        }) {
            Ok(_) => (),
            Err(err) => eprintln!("Failed to queue download {err}"),
        }
    }

    /// Download multiple files from the active server connection.
    fn mget(&mut self, args: &[String]) {
        if args.len() == 0 {
            eprintln!("Usage: mget <file1> ... <fileX>");
            return;
        }

        let Some(ref mut sender) = self.sender else {
            eprintln!("Start a connection with open <url> before calling mget");
            return;
        };

        println!("Downloading files {args:?}...");

        for (i, file_path) in args.iter().enumerate() {
            match sender.send(QftpCommand::Download {
                remote_path: file_path.clone(),
                local_save_path: format!("result{i}.dat"),
            }) {
                Ok(_) => (),
                Err(err) => eprintln!("Failed to queue download {err}"),
            }
        }
    }

    /// Upload a single file to the active server connection
    fn put(&mut self, args: &[String]) {
        if args.len() != 1 {
            eprintln!("Usage: put <file>");
            return;
        }

        let Some(ref mut sender) = self.sender else {
            eprintln!("Start a connection with open <url> before calling put");
            return;
        };

        let file_path = args[0].clone();

        println!("Uploading file {file_path}...");
        match sender.send(QftpCommand::Upload {
            remote_save_path: "result.dat".to_string(),
            local_path: file_path,
        }) {
            Ok(_) => (),
            Err(err) => eprintln!("Failed to queue download {err}"),
        }
    }
}
