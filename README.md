# qftp

⚡Blazingly fast `ftp` alternative, using the QUIC protocol.

<p align="center">
  <img src="./assets/qftp.png" alt="qftp logo">
</p>

## Compile

```bash
cargo build
```

## Generate X.509 certificate

This is required to run the server at the moment. Run this in the root of the project.

```bash
openssl req -x509 -newkey rsa:4096 -keyout cert.key -out cert.crt -days 365 -nodes -subj "/CN=localhost"
```

## Run

```bash
# Start server
cargo run -- server

# Start client
cargo run -- client

# In client repl
open 127.0.0.1 # Open server connection
get file1.txt # Download
mget file1.txt file2.txt # Download multiple
put file1.txt # Upload
```

## Docs

```bash
cargo doc --open
```
