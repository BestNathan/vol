# vol-agent-server Dockerfile (Debian slim runtime)
# =============================================================================
# Multi-stage build for the JSON-RPC agent service.
#
# Build args:
#   ROLE   — control-plane | data-plane (default: data-plane). Selects which
#            default config TOML is baked into the image at
#            /etc/vol-agent-server/agent-server.toml. The runtime config can
#            still be overridden at deploy time via --config <path>.
#   REGION — cn (default) | global. cn uses aliyun apt mirror + rsproxy.cn
#            for rustup and crates.io. global uses Debian/rustup/crates.io
#            official sources (required when building from networks that
#            can't reach the China mirrors, e.g. GitHub Actions runners).
#
# Build:
#   # Local (China network):
#   docker build --build-arg ROLE=control-plane -t vol-agent-server:cp-latest \
#     -f dockers/vol-agent-server.Dockerfile .
#   # CI / outside China:
#   docker build --build-arg ROLE=data-plane --build-arg REGION=global \
#     -t vol-agent-server:dp-latest -f dockers/vol-agent-server.Dockerfile .
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

# ── Base: Rust toolchain + cargo-chef + sccache + mold (shared by planner and builder) ─
FROM debian:bookworm-slim AS base

ARG REGION=cn

ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH \
    SCCACHE_CACHE_SIZE=10G \
    SCCACHE_IDLE_TIMEOUT=0 \
    CARGO_INCREMENTAL=0

RUN set -eux; \
    if [ "$REGION" = "cn" ]; then \
        sed -i 's|deb.debian.org|mirrors.aliyun.com|g' /etc/apt/sources.list.d/debian.sources; \
    fi; \
    apt-get update; \
    apt-get install -y --no-install-recommends \
        curl gcc g++ make cmake perl libssl-dev pkg-config ca-certificates git \
        clang; \
    rm -rf /var/lib/apt/lists/*; \
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

# Install sccache (Rust compile cache — huge speedup in CI)
RUN set -eux; \
    arch=$(uname -m); \
    case "$arch" in \
        x86_64) sccache_arch="x86_64-unknown-linux-musl" ;; \
        aarch64) sccache_arch="aarch64-unknown-linux-musl" ;; \
        *) echo "unsupported arch: $arch"; exit 1 ;; \
    esac; \
    ver="v0.9.1"; \
    url="https://github.com/mozilla/sccache/releases/download/${ver}/sccache-${ver}-${sccache_arch}.tar.gz"; \
    curl -fsSL "$url" | tar -xz -C /usr/local/bin --strip-components=1 "sccache-${ver}-${sccache_arch}/sccache"; \
    chmod +x /usr/local/bin/sccache; \
    sccache --version

# Install mold (fast linker — 5-10x faster than GNU ld)
RUN set -eux; \
    arch=$(uname -m); \
    case "$arch" in \
        x86_64) mold_arch="x86_64" ;; \
        aarch64) mold_arch="aarch64" ;; \
        *) echo "unsupported arch: $arch"; exit 1 ;; \
    esac; \
    ver="2.37.0"; \
    url="https://github.com/rui314/mold/releases/download/v${ver}/mold-${ver}-${mold_arch}-linux.tar.gz"; \
    curl -fsSL "$url" | tar -xz -C /usr/local --strip-components=1; \
    chmod +x /usr/local/bin/mold; \
    mold --version

RUN cargo install cargo-chef --locked
WORKDIR /app

# ── Planner: scan workspace → generate dependency recipe ──────────────────────
FROM base AS planner

COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/
COPY .cargo/ .cargo/

RUN --mount=type=cache,target=/root/.cache/sccache \
    cargo chef prepare --recipe-path recipe.json

# ── Builder: compile deps from recipe (CACHED), then build workspace ──────────
FROM base AS builder

# Environment for sccache (local disk cache) + mold linker
ENV RUSTC_WRAPPER=sccache \
    CARGO_NET_RETRY=10 \
    CARGO_HTTP_TIMEOUT=120

# recipe.json is <1KB — cached across any .rs source change
COPY --from=planner /app/recipe.json recipe.json
COPY Cargo.toml Cargo.lock ./

# Compile dependencies from recipe. CACHED by type=gha Docker layer cache.
# sccache cache mount persists across builds on the same runner.
RUN --mount=type=cache,target=/root/.cache/sccache \
    cargo chef cook --release --recipe-path recipe.json -p vol-agent-server

# Copy real source and build workspace crates
COPY crates/ ./crates/
COPY .cargo/ .cargo/

# Build with mold linker + sccache. The sccache cache is non-blocking —
# if the mount is empty (no prior cache), it just starts fresh.
RUN --mount=type=cache,target=/root/.cache/sccache \
    cargo build --release -p vol-agent-server \
        --config 'target.x86_64-unknown-linux-gnu.linker="clang"' \
        --config 'target.x86_64-unknown-linux-gnu.rustflags=["-C", "link-arg=-fuse-ld=mold"]' \
        --config 'target.aarch64-unknown-linux-gnu.linker="clang"' \
        --config 'target.aarch64-unknown-linux-gnu.rustflags=["-C", "link-arg=-fuse-ld=mold"]' \
    && strip /app/target/release/vol-agent-server \
    && sccache --show-stats

# ── Runtime ───────────────────────────────────────────────────────────────────
FROM debian:bookworm-slim

ARG ROLE=data-plane
ARG REGION=cn

RUN set -eux; \
    if [ "$REGION" = "cn" ]; then \
        sed -i 's|deb.debian.org|mirrors.aliyun.com|g' /etc/apt/sources.list.d/debian.sources; \
    fi; \
    apt-get update; \
    apt-get install -y --no-install-recommends ca-certificates; \
    rm -rf /var/lib/apt/lists/*; \
    addgroup --system --gid 1000 vol-agent; \
    adduser --system --uid 1000 --gid 1000 --no-create-home vol-agent

WORKDIR /app

COPY --from=builder /app/target/release/vol-agent-server /usr/local/bin/vol-agent-server
COPY configs/vol-agent-server.${ROLE}.toml /etc/vol-agent-server/agent-server.toml

ENV VOL_AGENT_SERVER_ROLE=${ROLE}

RUN mkdir -p /app/data && \
    chown -R vol-agent:vol-agent /app /etc/vol-agent-server

USER vol-agent:vol-agent

EXPOSE 3001

ENTRYPOINT ["/usr/local/bin/vol-agent-server"]
CMD ["--config", "/etc/vol-agent-server/agent-server.toml"]
