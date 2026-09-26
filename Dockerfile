# Build stage: Debian Bookworm with Rust
FROM rust:slim-bookworm AS builder

WORKDIR /usr/src/emusic

# Copy workspace manifest and source code
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/

# Build size-optimized release binary for emusic-server
RUN cargo build -p emusic-server --release

# Runtime stage: Minimal Debian Bookworm slim
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    sqlite3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /var/lib/emusic-server

# Copy compiled binary from builder
COPY --from=builder /usr/src/emusic/target/release/emusic-server /usr/local/bin/emusic-server

EXPOSE 8080

ENTRYPOINT ["emusic-server"]
CMD ["run", "--config", "/etc/emusic-server/server.toml"]
