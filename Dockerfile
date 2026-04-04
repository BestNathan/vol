# Stage 1: Build
FROM docker.1panel.live/library/rust:latest AS builder

WORKDIR /app

# Copy dependency definitions first for better caching
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/
COPY .cargo ./.cargo

# Build release binary
RUN cargo build --release -vv

# Stage 2: Runtime
FROM docker.1panel.live/library/debian:bookworm-slim

# Install CA certificates for HTTPS using Aliyun mirror
RUN sed -i 's|deb.debian.org|mirrors.aliyun.com|g' /etc/apt/sources.list.d/debian.sources && \
    apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

# Copy the binary from builder
COPY --from=builder /app/target/release/vol-monitor /usr/local/bin/vol-monitor

# config.toml is mounted via ConfigMap at runtime
WORKDIR /app

# Run the binary
ENTRYPOINT ["/usr/local/bin/vol-monitor"]
