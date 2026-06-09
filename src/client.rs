use crate::common;
use io::Write;
use rand::TryRng;
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

struct Client {
    sender: Option<mpsc::Sender<String>>,
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

        let (tx, rx) = mpsc::channel::<String>();

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
            rand::rng().try_fill_bytes(&mut scid_bytes).expect("Failed to generate connection ID");
            let scid = quiche::ConnectionId::from_vec(scid_bytes);

            let mut conn = quiche::connect(
                Some("localhost"),
                &scid,
                local_addr,
                server_addr,
                &mut config,
            ).expect("Failed to create quiche connection");

            let mut buf = [0; 65535];
            let mut out = [0; 65535];

            let mut next_stream_id: u64 = 0;

            loop {
                // Read MPSC channel
                if conn.is_established() {
                    while let Ok(message_to_send) = rx.try_recv() {
                        // Send the message and flag `fin` as true to signify the stream's payload is complete
                        match conn.stream_send(next_stream_id, message_to_send.as_bytes(), true) {
                            Ok(_bytes) => {
                                next_stream_id += 4; // Increment to next valid client bidi stream ID
                            }
                            Err(e) => {
                                eprintln!("[QUIC Background] Stream send error: {:?}", e);
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

        let file_path = args[0].as_str();

        println!("Downloading file {file_path}...");
        match sender.send(format!("get {file_path}").to_string()) {
            Ok(_) => (),
            Err(err) => eprintln!("Failed to send data {err}"),
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
        match sender.send(format!("mget {args:?}").to_string()) {
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

        let file_path = args[0].as_str();

        println!("Uploading file {file_path}...");
        match sender.send(format!("put {file_path}").to_string()) {
            Ok(_) => (),
            Err(err) => eprintln!("Failed to send data {err}"),
        }
    }
}
