# vol-agent-server Dockerfile (Alpine musl runtime, ~30MB image)
# =============================================================================
# Multi-stage build for the JSON-RPC agent service — minimal Alpine runtime.
#
# Build:
#   docker build -t vol-agent-server:alpine -f dockers/vol-agent-server.alpine.Dockerfile .
#
# Run:
#   docker run -d \
#     -p 3001:3001 \
#     -v $(pwd)/.agents:/app/.agents:ro \
#     -v $(pwd)/.mcp.json:/app/.mcp.json:ro \
#     -e ANTHROPIC_AUTH_TOKEN=sk-xxx \
#     vol-agent-server:alpine
# =============================================================================

# ── Stage 1: Builder (Alpine + rsproxy) ─────────────────────────────────────
FROM alpine:3.21 AS builder

ENV RUSTUP_DIST_SERVER=https://rsproxy.cn \
    RUSTUP_UPDATE_ROOT=https://rsproxy.cn/rustup \
    RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    HOME=/root \
    PATH=/usr/local/cargo/bin:$PATH

# Install build deps via Alibaba mirror, then Rust via rsproxy
RUN sed -i 's/dl-cdn.alpinelinux.org/mirrors.aliyun.com/g' /etc/apk/repositories && \
    apk add --no-cache \
    curl gcc g++ musl-dev pkgconfig openssl-dev openssl-libs-static \
    perl make git binutils && \
    curl --proto '=https' --tlsv1.2 -sSf https://rsproxy.cn/rustup-init.sh | sh -s -- -y

# Copy cargo mirror configuration
COPY .cargo/config.toml .cargo/config.toml

WORKDIR /app

# Copy workspace source
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/

# Build and strip the agent-server binary (musl = static binary)
RUN cargo build --release -p vol-agent-server && \
    strip /app/target/release/vol-agent-server

# ── Stage 2: Runtime ────────────────────────────────────────────────────────
FROM alpine:3.21

# Install CA certificates for HTTPS
RUN apk add --no-cache ca-certificates

WORKDIR /app

# Copy the statically-linked binary
COPY --from=builder /app/target/release/vol-agent-server /usr/local/bin/vol-agent-server

# Create data directory
RUN mkdir -p /app/data

EXPOSE 3001

ENTRYPOINT ["/usr/local/bin/vol-agent-server"]
