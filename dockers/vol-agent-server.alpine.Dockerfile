# vol-agent-server Dockerfile (Alpine musl runtime, ~30MB image)
# =============================================================================
# Multi-stage build for the JSON-RPC agent service — minimal Alpine runtime.
#
# Build args:
#   ROLE   — control-plane | data-plane (default: data-plane). Selects which
#            default config TOML is baked into the image.
#   REGION — cn (default) | global. cn uses aliyun apk mirror + rsproxy.cn
#            for rustup and crates.io. global uses dl-cdn.alpinelinux.org and
#            official rustup/crates.io sources (for GitHub Actions runners).
#
# Build:
#   # Local (China network):
#   docker build --build-arg ROLE=data-plane -t vol-agent-server:alpine \
#     -f dockers/vol-agent-server.alpine.Dockerfile .
#   # CI / outside China:
#   docker build --build-arg ROLE=data-plane --build-arg REGION=global \
#     -t vol-agent-server:alpine -f dockers/vol-agent-server.alpine.Dockerfile .
#
# Run:
#   docker run -d \
#     -p 3001:3001 \
#     -v $(pwd)/.agents:/app/.agents:ro \
#     -v $(pwd)/.mcp.json:/app/.mcp.json:ro \
#     -e ANTHROPIC_AUTH_TOKEN=sk-xxx \
#     vol-agent-server:alpine
# =============================================================================

# ── Base: Rust toolchain + cargo-chef (shared by planner and builder) ────────
FROM alpine:3.21 AS base

ARG REGION=cn

ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    HOME=/root \
    PATH=/usr/local/cargo/bin:$PATH

RUN set -eux; \
    if [ "$REGION" = "cn" ]; then \
        sed -i 's/dl-cdn.alpinelinux.org/mirrors.aliyun.com/g' /etc/apk/repositories; \
    fi; \
    apk add --no-cache \
        curl gcc g++ musl-dev pkgconfig openssl-dev openssl-libs-static \
        perl make git binutils; \
    if [ "$REGION" = "cn" ]; then \
        export RUSTUP_DIST_SERVER=https://rsproxy.cn; \
        export RUSTUP_UPDATE_ROOT=https://rsproxy.cn/rustup; \
        curl --proto '=https' --tlsv1.2 -sSf https://rsproxy.cn/rustup-init.sh \
            | sh -s -- -y --default-toolchain stable; \
        mkdir -p "$CARGO_HOME"; \
        printf '%s\n' \
            '[source.crates-io]' \
            'replace-with = "rsproxy-sparse"' \
            '[source.rsproxy-sparse]' \
            'registry = "sparse+https://rsproxy.cn/index/"' \
            '[net]' \
            'git-fetch-with-cli = true' \
            > "$CARGO_HOME/config.toml"; \
    else \
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
            | sh -s -- -y --default-toolchain stable; \
    fi; \
    cargo --version

RUN cargo install cargo-chef --locked
WORKDIR /app

# ── Planner: scan workspace → generate dependency recipe ─────────────────────
FROM base AS planner

COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/
COPY .cargo/ .cargo/

RUN cargo chef prepare --recipe-path recipe.json

# ── Builder: compile deps from recipe (CACHED), then build workspace ──────────
FROM base AS builder

COPY --from=planner /app/recipe.json recipe.json
COPY Cargo.toml Cargo.lock ./

ENV CARGO_NET_RETRY=10 \
    CARGO_HTTP_TIMEOUT=120
RUN cargo chef cook --release --recipe-path recipe.json -p vol-agent-server

COPY crates/ ./crates/
COPY .cargo/ .cargo/

# Build and strip the agent-server binary (musl = static binary)
RUN cargo build --release -p vol-agent-server && \
    strip /app/target/release/vol-agent-server

# ── Runtime ────────────────────────────────────────────────────────────────────
FROM alpine:3.21

ARG ROLE=data-plane
ARG REGION=cn

RUN set -eux; \
    if [ "$REGION" = "cn" ]; then \
        sed -i 's/dl-cdn.alpinelinux.org/mirrors.aliyun.com/g' /etc/apk/repositories; \
    fi; \
    apk add --no-cache ca-certificates; \
    addgroup -S -g 1000 vol-agent; \
    adduser -S -u 1000 -G vol-agent -h /app -D vol-agent

WORKDIR /app

# Copy the statically-linked binary
COPY --from=builder /app/target/release/vol-agent-server /usr/local/bin/vol-agent-server
COPY configs/vol-agent-server.${ROLE}.toml /etc/vol-agent-server/agent-server.toml

ENV VOL_AGENT_SERVER_ROLE=${ROLE}

RUN mkdir -p /app/data && \
    chown -R vol-agent:vol-agent /app /etc/vol-agent-server

# Run as non-root user
USER vol-agent:vol-agent

EXPOSE 3001

ENTRYPOINT ["/usr/local/bin/vol-agent-server"]
CMD ["--config", "/etc/vol-agent-server/agent-server.toml"]
