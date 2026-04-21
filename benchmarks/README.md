# File transfer benchmarks

This directory contains setup of two `debian:13-slim` based Docker images:
- vsftpd server
- ftp client

These are used for running file transfer benchmarks.
You can start the containers like so:
```bash
docker compose up --build -d
```
