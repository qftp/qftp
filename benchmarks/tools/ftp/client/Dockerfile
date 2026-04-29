FROM debian:13-slim

# See https://docs.docker.com/build/building/best-practices/#apt-get
RUN apt-get update && apt-get install -y \
    ftp \
    iputils-ping \
    netbase \
    && rm -rf /var/lib/apt/lists/*

COPY --chmod=600 .netrc /root/.netrc

RUN mkdir -p /ftp-files/uploads/

WORKDIR /ftp-files

CMD ["sleep", "infinity"]
