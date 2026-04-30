FROM debian:13-slim

# See https://docs.docker.com/build/building/best-practices/#apt-get
RUN apt-get update && apt-get install -y \
    nginx \
    openssl \
    iputils-ping \
    && rm -rf /var/lib/apt/lists/*

RUN mkdir -p /etc/nginx/ssl /var/www/html/files

RUN chown -R www-data:www-data /var/www/html

# HTTP/3 requires a certificate
RUN openssl req -x509 -nodes -days 365 -newkey rsa:2048 \
    -keyout /etc/nginx/ssl/server.key \
    -out /etc/nginx/ssl/server.crt \
    -subj "/CN=http3-server"

COPY nginx.conf /etc/nginx/nginx.conf

EXPOSE 443/udp

CMD ["nginx", "-g", "daemon off;"]
