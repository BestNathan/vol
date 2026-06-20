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
    curl --proto '=https' --tlsv1.2 -sSf "https://sh.rustup.rs" \
        | sh -s -- -y --default-toolchain stable --profile minimal; \
    rustup target add wasm32-unknown-unknown; \
    # Install Dioxus CLI (pinned version matching the project)
    cargo install dioxus-cli --version 0.6.3 --locked; \
    # Install cargo-chef for dependency caching
    cargo install cargo-chef --locked; \
    # Copy .cargo/config.toml for rsproxy.cn mirror (cn region)
    if [ "$REGION" = "cn" ]; then \
        mkdir -p /usr/local/cargo; \
        cat > /usr/local/cargo/config.toml <<'CARGO_EOF'
[source.crates-io]
replace-with = 'rsproxy-sparse'

[source.rsproxy-sparse]
registry = "sparse+https://rsproxy.cn/crates.io-index/"

[registries.rsproxy]
index = "https://rsproxy.cn/crates.io-index"

[net]
git-fetch-with-cli = true
CARGO_EOF
    fi; \
    apt-get clean; \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

# ── Planner: generate cargo-chef recipe ──────────────────────────────────────
FROM base AS planner

# Copy workspace Cargo.toml + crate Cargo.tomls for recipe generation
COPY Cargo.toml Cargo.lock ./
COPY crates/vol-llm-ui/Cargo.toml crates/vol-llm-ui/Cargo.toml
COPY crates/vol-llm-core/Cargo.toml crates/vol-llm-core/Cargo.toml
COPY crates/vol-llm-provider/Cargo.toml crates/vol-llm-provider/Cargo.toml
COPY crates/vol-llm-tool/Cargo.toml crates/vol-llm-tool/Cargo.toml
COPY crates/vol-llm-agent/Cargo.toml crates/vol-llm-agent/Cargo.toml
COPY crates/vol-llm-mcp/Cargo.toml crates/vol-llm-mcp/Cargo.toml
COPY crates/vol-llm-runtime/Cargo.toml crates/vol-llm-runtime/Cargo.toml
COPY crates/vol-llm-skill/Cargo.toml crates/vol-llm-skill/Cargo.toml
COPY crates/vol-llm-task/Cargo.toml crates/vol-llm-task/Cargo.toml
COPY crates/vol-session/Cargo.toml crates/vol-session/Cargo.toml
COPY crates/vol-llm-context/Cargo.toml crates/vol-llm-context/Cargo.toml
COPY crates/vol-llm-memory/Cargo.toml crates/vol-llm-memory/Cargo.toml
COPY crates/vol-llm-sandbox/Cargo.toml crates/vol-llm-sandbox/Cargo.toml
COPY crates/vol-llm-wiki/Cargo.toml crates/vol-llm-wiki/Cargo.toml

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
