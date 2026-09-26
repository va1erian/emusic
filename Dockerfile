# Multi-stage Dockerfile for emusic-server
FROM rust:1.85-slim-bookworm AS builder

WORKDIR /usr/src/emusic

# Install build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

# Build release binary for emusic-server
RUN cargo build --release -p emusic-server

# Minimal runtime image
FROM debian:bookworm-slim AS runtime

WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/emusic/target/release/emusic-server /usr/local/bin/emusic-server

# Expose internal HTTP port for Cosmos Cloud / reverse proxy
EXPOSE 8080

ENV RUST_LOG=info,emusic_server=info
ENV EMUSIC_HOST=0.0.0.0
ENV EMUSIC_PORT=8080
ENV EMUSIC_DATA_DIR=/var/lib/emusic-server
ENV EMUSIC_MUSIC_DIR=/media/music

ENTRYPOINT ["emusic-server"]
CMD ["serve"]
