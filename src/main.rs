use std::env;
use std::process;

mod client;
mod server;

fn main() {
    let mode = env::args().nth(1);

    match mode.as_deref() {
        Some("client") => client::run(),
        Some("server") => server::run(),
        _ => {
            print_help();
            process::exit(1);
        }
    }
}

fn print_help() {
    println!(
        "qftp - QUIC File Transfer Protocol
Usage: qftp <mode>

<mode>:
  client - run the qftp client
  server - start the qftp server"
    );
}
