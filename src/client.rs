use crate::common;
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

enum Operation {
    Get(String),
    Mget(Vec<String>),
    Put(String),
}

struct Client {
    sender: Option<mpsc::Sender<Operation>>,
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

        let (tx, rx) = mpsc::channel::<Operation>();

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

            loop {
                // Read MPSC channel
                if conn.is_established() {
                    while let Ok(operation) = rx.try_recv() {
                        match operation {
                            Operation::Get(file_path) => {
                                match conn.stream_send(next_stream_id, file_path.as_bytes(), true) {
                                    Ok(_bytes) => {
                                        next_stream_id += 4; // Increment to next valid client bidi stream ID
                                    }
                                    Err(e) => {
                                        eprintln!("[QUIC Background] Stream send error: {:?}", e);
                                    }
                                }
                            }
                            Operation::Mget(_) => (),
                            Operation::Put(_) => (),
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
                        let mut chunk_buf = [0; 8192]; // 8KB read window

                        match conn.stream_recv(stream_id, &mut chunk_buf) {
                            Ok((read_len, fin)) if read_len > 0 => {
                                let mut data_start = 0;

                                // If this is the very first packet on this stream, parse our protocol header
                                if !active_sinks.contains_key(&stream_id) {
                                    let status_code = chunk_buf[0];
                                    data_start = 1; // Skip the protocol prefix byte

                                    if status_code == 0x00 {
                                        let err_msg = std::str::from_utf8(&chunk_buf[1..read_len])
                                            .unwrap_or("Error");
                                        eprintln!("[Client] Server failure: {}", err_msg);
                                        active_sinks.insert(
                                            stream_id,
                                            (File::open("/dev/null").unwrap(), true),
                                        ); // dummy error sink
                                        continue;
                                    } else if status_code == 0x01 {
                                        // Create a unique local file for this specific stream download
                                        let save_path =
                                            format!("download_stream_{}.dat", stream_id);
                                        let file = OpenOptions::new()
                                            .create(true)
                                            .append(true)
                                            .open(&save_path)
                                            .unwrap();
                                        println!(
                                            "[Client] Stream {} verified. Writing to {}",
                                            stream_id, save_path
                                        );
                                        active_sinks.insert(stream_id, (file, false));
                                    }
                                }

                                // Write this payload chunk directly to disk
                                if let Some((file, is_error)) = active_sinks.get_mut(&stream_id) {
                                    if !*is_error && read_len > data_start {
                                        file.write_all(&chunk_buf[data_start..read_len]).unwrap();
                                    }
                                }

                                if fin {
                                    println!(
                                        "[Client] Stream {} finished downloading cleanly.",
                                        stream_id
                                    );
                                    active_sinks.remove(&stream_id);
                                }
                            }
                            Ok((_, fin)) => {
                                if fin {
                                    active_sinks.remove(&stream_id);
                                }
                            }
                            Err(quiche::Error::Done) => {}
                            Err(e) => {
                                eprintln!("[Client] Stream read error: {:?}", e);
                                active_sinks.remove(&stream_id);
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
        match sender.send(Operation::Get(file_path)) {
            Ok(_) => (),
            Err(err) => eprintln!("Failed to send operation {err}"),
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
        match sender.send(Operation::Mget(args.to_vec())) {
            Ok(_) => (),
            Err(err) => eprintln!("Failed to send data {err}"),
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
        match sender.send(Operation::Put(file_path)) {
            Ok(_) => (),
            Err(err) => eprintln!("Failed to send data {err}"),
        }
    }
}
