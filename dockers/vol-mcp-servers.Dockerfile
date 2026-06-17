# vol-mcp-servers Dockerfile (Debian slim runtime)
# =============================================================================
# Multi-stage build for binaries from the vol-mcp-servers crate.
#
# Build args:
#   BIN    — MCP binary to build and run (default: docs-rs-mcp)
#   REGION — cn (default) | global. cn uses aliyun apt mirror + rsproxy.cn
#            for rustup and crates.io. global uses Debian/rustup/crates.io
#            official sources for GitHub Actions runners.
#
# Build:
#   docker build --build-arg BIN=docs-rs-mcp --build-arg REGION=global \
#     -f dockers/vol-mcp-servers.Dockerfile -t docs-rs-mcp:local .
#
# Run:
#   docker run --rm -p 8080:8080 docs-rs-mcp:local --http 0.0.0.0:8080
# =============================================================================

FROM debian:bookworm-slim AS builder

ARG BIN=docs-rs-mcp
ARG REGION=cn

ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH

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

RUN set -eux; \
    for toml_path in crates/*/Cargo.toml; do \
        crate_dir="$(dirname "$toml_path")"; \
        src_dir="${crate_dir}/src"; \
        mkdir -p "$src_dir"; \
        if [ "$(basename "$crate_dir")" = "vol-mcp-servers" ]; then \
            echo 'fn main() { println!("dummy"); }' > "${src_dir}/main.rs"; \
        else \
            echo '#![allow(unused)]' > "${src_dir}/lib.rs"; \
        fi; \
    done

ENV CARGO_NET_RETRY=10 \
    CARGO_HTTP_TIMEOUT=120

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release -p vol-mcp-servers --bin "${BIN}"

# ── Phase 2: restore real source and build final binary ──────────────────────

COPY crates/ ./crates/
COPY .cargo/ .cargo/

RUN cargo build --release -p vol-mcp-servers --bin "${BIN}" && \
    strip "/app/target/release/${BIN}"

FROM debian:bookworm-slim

ARG BIN=docs-rs-mcp
ARG REGION=cn

RUN set -eux; \
    if [ "$REGION" = "cn" ]; then \
        sed -i 's|deb.debian.org|mirrors.aliyun.com|g' /etc/apt/sources.list.d/debian.sources; \
    fi; \
    apt-get update; \
    apt-get install -y --no-install-recommends ca-certificates; \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder "/app/target/release/${BIN}" /usr/local/bin/mcp-server

EXPOSE 8080

ENTRYPOINT ["/usr/local/bin/mcp-server"]
