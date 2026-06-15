# vol-agent-server Dockerfile (Debian slim runtime)
# =============================================================================
# Multi-stage build for the JSON-RPC agent service.
#
# Build args:
#   ROLE — control-plane | data-plane (default: data-plane). Selects which
#          default config TOML is baked into the image at
#          /etc/vol-agent-server/agent-server.toml. The runtime config can
#          still be overridden at deploy time via --config <path>.
#
# Build:
#   docker build --build-arg ROLE=control-plane -t vol-agent-server:cp-latest \
#     -f dockers/vol-agent-server.Dockerfile .
#   docker build --build-arg ROLE=data-plane    -t vol-agent-server:dp-latest \
#     -f dockers/vol-agent-server.Dockerfile .
#
# Run:
#   docker run -d \
#     -p 3001:3001 \
#     -v $(pwd)/.agents:/app/.agents:ro \
#     -v $(pwd)/.mcp.json:/app/.mcp.json:ro \
#     -e ANTHROPIC_AUTH_TOKEN=sk-xxx \
#     vol-agent-server:latest
#
# Or with custom config:
#   docker run ... -v $(pwd)/my-config.toml:/app/agent-server.toml:ro \
#     vol-agent-server:latest --config /app/agent-server.toml
# =============================================================================

# ── Stage 1: Builder (same Debian as runtime → matching glibc) ──────────────
FROM debian:bookworm-slim AS builder

ENV RUSTUP_DIST_SERVER=https://rsproxy.cn \
    RUSTUP_UPDATE_ROOT=https://rsproxy.cn/rustup \
    RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH

# Install build dependencies via Aliyun mirror, then Rust via rsproxy
RUN sed -i 's|deb.debian.org|mirrors.aliyun.com|g' /etc/apt/sources.list.d/debian.sources && \
    apt-get update && apt-get install -y --no-install-recommends \
    curl gcc g++ libssl-dev pkg-config ca-certificates git && \
    rm -rf /var/lib/apt/lists/* && \
    curl --proto '=https' --tlsv1.2 -sSf https://rsproxy.cn/rustup-init.sh | sh -s -- -y

# Copy cargo mirror config for crates.io access via rsproxy
COPY .cargo/config.toml .cargo/config.toml

WORKDIR /app

# Copy dependency manifests first for layer caching
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/

# Build and strip the agent-server binary
RUN cargo build --release -p vol-agent-server && \
    strip /app/target/release/vol-agent-server

# ── Stage 2: Runtime ────────────────────────────────────────────────────────
FROM debian:bookworm-slim

ARG ROLE=data-plane

# Install CA certificates for HTTPS
RUN sed -i 's|deb.debian.org|mirrors.aliyun.com|g' /etc/apt/sources.list.d/debian.sources && \
    apt-get update && apt-get install -y --no-install-recommends ca-certificates && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the binary
COPY --from=builder /app/target/release/vol-agent-server /usr/local/bin/vol-agent-server

# Bake in the role-specific default config. ROLE=control-plane | data-plane.
COPY configs/vol-agent-server.${ROLE}.toml /etc/vol-agent-server/agent-server.toml

ENV VOL_AGENT_SERVER_ROLE=${ROLE}

# Create data directory
RUN mkdir -p /app/data

EXPOSE 3001

ENTRYPOINT ["/usr/local/bin/vol-agent-server"]
CMD ["--config", "/etc/vol-agent-server/agent-server.toml"]
