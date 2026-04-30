FROM debian:13-slim

# See https://docs.docker.com/build/building/best-practices/#apt-get
# RUN apt-get update && apt-get install -y curl ca-certificates && \
#     rm -rf /var/lib/apt/lists/*
RUN apt-get update && apt-get install -y \
    curl \
    iputils-ping \
    && rm -rf /var/lib/apt/lists/*

RUN mkdir -p /files/uploads

WORKDIR /files/uploads

CMD ["sleep", "infinity"]
