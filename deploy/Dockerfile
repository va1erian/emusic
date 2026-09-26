# syntax=docker/dockerfile:1
#
# Multi-stage build for emusic-server. Build from the repository root:
#
#   docker build -f deploy/Dockerfile -t va1erian/emusic-server .
#
# The bundled SQLite and the rustls provider are compiled from source, so the
# builder image needs a C toolchain and CMake.

FROM rust:1-bookworm AS builder
RUN apt-get update \
    && apt-get install -y --no-install-recommends cmake perl \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY . .
RUN cargo build --release --locked -p emusic-server

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --home /var/lib/emusic-server emusic \
    && mkdir -p /var/lib/emusic-server /etc/emusic-server /media/music \
    && chown emusic:emusic /var/lib/emusic-server

COPY --from=builder /src/target/release/emusic-server /usr/local/bin/emusic-server

USER emusic
EXPOSE 8080
VOLUME ["/var/lib/emusic-server"]
ENTRYPOINT ["/usr/local/bin/emusic-server"]
CMD ["--config", "/etc/emusic-server/server.toml"]
