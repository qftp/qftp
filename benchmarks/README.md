# File transfer benchmarks

This directory contains setup of two `debian:13-slim` based Docker containers, using `docker compose`:
- ftp-server - based on `vsftpd`
- ftp-client

The default ftp credentials are:
- username: `benchmark`
- password: `benchmark`

## Container setup

The containers are used for running file transfer benchmarks.
You can start them like so:
```bash
docker compose up --build -d
```

To later stop them, use:
```bash
docker compose down
```

## Interactive use

To start an interactive ftp session from the client container, run:
```bash
docker compose exec ftp-client ftp ftp-server
```

## Applying `tc` commands

On the host you will find 3 new network interfaces (a bridge and 2 `veth` interfaces). You can apply `tc` commands to those.
