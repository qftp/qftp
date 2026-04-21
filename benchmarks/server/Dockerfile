FROM debian:13-slim

RUN apt-get update && \
    apt-get install -y vsftpd && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*

RUN useradd -m benchmark && \
    echo "benchmark:benchmark" | chpasswd

COPY vsftpd.conf /etc/vsftpd.conf

EXPOSE 21 21100-21110

CMD ["vsftpd"]
