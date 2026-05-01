# File transfer benchmarks

This directory contains setup of multiple file transfer tools done using Docker Compose. Per each tool, 2 Docker containers are defined (the server and client) and based on `debian:13-slim`. The `Dockerfile` files are in the `tools` directory. Additional directories are created upon starting the containers to house the download and upload files.

Tests and testing utilities are written in Python and stored in the `tests` directory.

## Setup directories

Before starting Docker you need to create some directories first, which will allow you to run tests without `sudo`:
```bash
mkdir download-files upload-files
```

## Manage tool containers

You can start them like so:
```bash
docker compose up --build -d
```

To later stop them, use:
```bash
docker compose down
```

## Run tests

```bash
python tests/test1.py
```
