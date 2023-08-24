FROM redis:7.2-bookworm
RUN apt-get update && apt-get -y upgrade && apt-get autoremove
# run as non-root user
RUN useradd --create-home pixum --shell /bin/false
WORKDIR /home/pixum
USER pixum
COPY redis.conf /usr/local/etc/redis/redis.conf
ENTRYPOINT [ "redis-server", "/usr/local/etc/redis/redis.conf" ]