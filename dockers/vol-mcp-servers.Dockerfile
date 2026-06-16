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

COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/

ENV CARGO_NET_RETRY=10 \
    CARGO_HTTP_TIMEOUT=120
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
