use crate::common;
use io::Write;
use std::io;
use std::net::{SocketAddr, IpAddr, Ipv4Addr};

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
    conn: Option<quiche::Connection>,
}

impl Client {
    fn new() -> Client {
        return Client { conn: None };
    }

    /// Open a new connection to a qftp server.
    fn open(&mut self, args: &[String]) {
        if args.len() != 1 {
            eprintln!("Usage: open <url>");
            return;
        }

        let url = args[0].as_str();

        let mut config = common::new_quiche_config().expect("Failed to create quiche config");
        let scid = quiche::ConnectionId::default();
        let local = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), common::QFTP_PORT);
        let Ok(dest_ip) = url.parse::<IpAddr>() else {
            eprint!("Failed to parse url");
            return;
        };
        let peer = SocketAddr::new(dest_ip, common::QFTP_PORT);

        self.conn = Some(
            quiche::connect(Some(url), &scid, local, peer, &mut config).expect("Failed to connect"),
        );
    }

    /// Download a single file from the active server connection.
    fn get(&mut self, args: &[String]) {
        if args.len() != 1 {
            eprintln!("Usage: get <file>");
            return;
        }

        let Some(ref mut conn) = self.conn else {
            eprintln!("Start a connection with open <url> before calling get");
            return;
        };

        let file_path = args[0].as_str();

        if conn.is_established() {
            println!("Downloading file {file_path}...");
            match conn.stream_send(0, format!("get {file_path}").as_bytes(), true) {
                Ok(_) => (),
                Err(err) => eprintln!("Failed to send message {err}"),
            }
        }
    }

    /// Download multiple files from the active server connection.
    fn mget(&mut self, args: &[String]) {
        if args.len() == 0 {
            eprintln!("Usage: mget <file1> ... <fileX>");
            return;
        }

        let Some(ref mut conn) = self.conn else {
            eprintln!("Start a connection with open <url> before calling mget");
            return;
        };

        if conn.is_established() {
            println!("Downloading files {args:?}...");
            match conn.stream_send(0, format!("mget {args:?}").as_bytes(), true) {
                Ok(_) => (),
                Err(err) => eprintln!("Failed to send message {err}"),
            }
        }
    }

    /// Upload a single file to the active server connection
    fn put(&mut self, args: &[String]) {
        if args.len() != 1 {
            eprintln!("Usage: put <file>");
            return;
        }

        let Some(ref mut conn) = self.conn else {
            eprintln!("Start a connection with open <url> before calling put");
            return;
        };

        let file_path = args[0].as_str();

        if conn.is_established() {
            println!("Uploading file {file_path}...");
            match conn.stream_send(0, format!("put {file_path}").as_bytes(), true) {
                Ok(_) => (),
                Err(err) => eprintln!("Failed to send message {err}"),
            }
        }
    }
}
