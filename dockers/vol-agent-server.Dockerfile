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

# ── Stage 1: Builder (same Debian as runtime → matching glibc) ──────────────
FROM debian:bookworm-slim AS builder

ARG REGION=cn

ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH

# Install build deps + Rust toolchain. Mirrors are toggled by REGION.
RUN set -eux; \
    if [ "$REGION" = "cn" ]; then \
        sed -i 's|deb.debian.org|mirrors.aliyun.com|g' /etc/apt/sources.list.d/debian.sources; \
    fi; \
    apt-get update; \
    apt-get install -y --no-install-recommends \
        curl gcc g++ make cmake perl libssl-dev pkg-config ca-certificates git; \
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

WORKDIR /app

# ── Phase 1: build dependencies (cached when Cargo.toml/Lock unchanged) ──
# Copy only Cargo.toml manifests — NOT source — so this layer is cacheable.
COPY Cargo.toml Cargo.lock ./
COPY \
  crates/md-frontmatter/Cargo.toml crates/md-frontmatter/ \
  crates/vol-alert/Cargo.toml crates/vol-alert/ \
  crates/vol-config/Cargo.toml crates/vol-config/ \
  crates/vol-core/Cargo.toml crates/vol-core/ \
  crates/vol-datasource/Cargo.toml crates/vol-datasource/ \
  crates/vol-deribit/Cargo.toml crates/vol-deribit/ \
  crates/vol-engine/Cargo.toml crates/vol-engine/ \
  crates/vol-eventbus/Cargo.toml crates/vol-eventbus/ \
  crates/vol-llm-agent-protocol/Cargo.toml crates/vol-llm-agent-protocol/ \
  crates/vol-llm-agent/Cargo.toml crates/vol-llm-agent/ \
  crates/vol-llm-agents/Cargo.toml crates/vol-llm-agents/ \
  crates/vol-llm-context/Cargo.toml crates/vol-llm-context/ \
  crates/vol-llm-core/Cargo.toml crates/vol-llm-core/ \
  crates/vol-llm-mcp/Cargo.toml crates/vol-llm-mcp/ \
  crates/vol-llm-memory/Cargo.toml crates/vol-llm-memory/ \
  crates/vol-llm-observability/Cargo.toml crates/vol-llm-observability/ \
  crates/vol-llm-provider/Cargo.toml crates/vol-llm-provider/ \
  crates/vol-llm-runtime/Cargo.toml crates/vol-llm-runtime/ \
  crates/vol-llm-sandbox/Cargo.toml crates/vol-llm-sandbox/ \
  crates/vol-llm-skill/Cargo.toml crates/vol-llm-skill/ \
  crates/vol-llm-task/Cargo.toml crates/vol-llm-task/ \
  crates/vol-llm-tdengine/Cargo.toml crates/vol-llm-tdengine/ \
  crates/vol-llm-tool/Cargo.toml crates/vol-llm-tool/ \
  crates/vol-llm-tools-builtin/Cargo.toml crates/vol-llm-tools-builtin/ \
  crates/vol-llm-wiki/Cargo.toml crates/vol-llm-wiki/ \
  crates/vol-mcp-servers/Cargo.toml crates/vol-mcp-servers/ \
  crates/vol-notification/Cargo.toml crates/vol-notification/ \
  crates/vol-observability/Cargo.toml crates/vol-observability/ \
  crates/vol-rules/Cargo.toml crates/vol-rules/ \
  crates/vol-session/Cargo.toml crates/vol-session/ \
  crates/vol-tdengine/Cargo.toml crates/vol-tdengine/ \
  crates/vol-tracing/Cargo.toml crates/vol-tracing/

# Create minimal dummy source so cargo only compiles dependencies.
# Real source is restored in Phase 2.
RUN set -eux; \
    for toml_path in crates/*/Cargo.toml; do \
        crate_dir="$(dirname "$toml_path")"; \
        src_dir="${crate_dir}/src"; \
        mkdir -p "$src_dir"; \
        if [ "$(basename "$crate_dir")" = "vol-agent-server" ]; then \
            echo 'fn main() { println!("dummy"); }' > "${src_dir}/main.rs"; \
        else \
            echo '#![allow(unused)]' > "${src_dir}/lib.rs"; \
        fi; \
    done

ENV CARGO_NET_RETRY=10 \
    CARGO_HTTP_TIMEOUT=120

# Compile all dependencies — cached when Cargo.toml/Cargo.lock unchanged
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release -p vol-agent-server

# ── Phase 2: restore real source and build final binary ──────────────────────

COPY crates/ ./crates/
COPY .cargo/ .cargo/

RUN cargo build --release -p vol-agent-server && \
    strip /app/target/release/vol-agent-server

# ── Stage 2: Runtime ────────────────────────────────────────────────────────
FROM debian:bookworm-slim

ARG ROLE=data-plane
ARG REGION=cn

# Install CA certificates for HTTPS
RUN set -eux; \
    if [ "$REGION" = "cn" ]; then \
        sed -i 's|deb.debian.org|mirrors.aliyun.com|g' /etc/apt/sources.list.d/debian.sources; \
    fi; \
    apt-get update; \
    apt-get install -y --no-install-recommends ca-certificates; \
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
