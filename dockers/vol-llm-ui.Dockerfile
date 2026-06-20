# vol-llm-ui Dockerfile (Dioxus WASM → nginx)
# =============================================================================
# Multi-stage build for the web frontend.
#
# Build args:
#   REGION — cn (default) | global. cn uses rsproxy.cn for rustup/crates.
#            global uses official sources (required for GitHub Actions runners).
#
# Build:
#   # Local (China network):
#   docker build -t vol-llm-ui:latest -f dockers/vol-llm-ui.Dockerfile .
#   # CI / outside China:
#   docker build --build-arg REGION=global -t vol-llm-ui:latest \
#     -f dockers/vol-llm-ui.Dockerfile .
# =============================================================================

# ── Base: Rust toolchain + wasm32 target + Dioxus CLI ────────────────────────
FROM debian:bookworm-slim AS base

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
    rustup target add wasm32-unknown-unknown; \
    cargo --version

# Install cargo-chef for dependency caching
RUN cargo install cargo-chef --locked

# Install Dioxus CLI (pinned version matching the project)
RUN cargo install dioxus-cli --version 0.6.3 --locked

WORKDIR /app

# ── Planner: generate cargo-chef recipe ──────────────────────────────────────
FROM base AS planner

# Copy workspace Cargo.toml + all crate Cargo.tomls for recipe generation
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/
COPY .cargo/ .cargo/

RUN cargo chef prepare --recipe-path recipe.json

# ── Builder: cook deps + build WASM ──────────────────────────────────────────
FROM base AS builder

# Cook dependencies (cached layer)
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --target wasm32-unknown-unknown \
    --package vol-llm-ui --bin vol-llm-ui-web --recipe-path recipe.json

# Copy source and build
COPY . .
RUN dx build --release --package vol-llm-ui --bin vol-llm-ui-web \
    --no-default-features --features web

# ── Runtime: nginx + static files ────────────────────────────────────────────
FROM nginx:1.27-alpine

# Copy nginx config
COPY dockers/nginx-frontend.conf /etc/nginx/conf.d/default.conf

# Copy Dioxus build output
COPY --from=builder /app/target/dx/vol-llm-ui-web/release/web/public/ /usr/share/nginx/html/

EXPOSE 80

CMD ["nginx", "-g", "daemon off;"]
