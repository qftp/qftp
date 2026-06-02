use io::Write;
use std::io;

pub fn run() {
    println!("qftp client - type ? for help");

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
            "open" => open(args),
            "get" => get(args),
            "mget" => mget(args),
            "put" => put(args),
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

/// Open a new connection to a qftp server.
fn open(args: &[String]) {
    if args.len() != 1 {
        eprintln!("Usage: open <url>");
        return;
    }

    let url = args[0].as_str();
    println!("Called open with {url}");
}

/// Download a single file from the active server connection.
fn get(args: &[String]) {
    if args.len() != 1 {
        eprintln!("Usage: get <file>");
        return;
    }

    let file_path = args[0].as_str();
    println!("Called get with {file_path}");
}

/// Download multiple files from the active server connection.
fn mget(args: &[String]) {
    if args.len() == 0 {
        eprintln!("Usage: mget <file1> ... <fileX>");
        return;
    }

    println!("Called mget with {args:?}");
}

/// Upload a single file to the active server connection
fn put(args: &[String]) {
    if args.len() != 1 {
        eprintln!("Usage: put <file>");
        return;
    }

    let file_path = args[0].as_str();
    println!("Called put with {file_path}");
}
