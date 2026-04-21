FROM debian:13-slim

RUN apt-get update && \
    apt-get install -y ftp && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*

RUN mkdir /ftp-files

WORKDIR /ftp-files

CMD ["sleep", "infinity"]
