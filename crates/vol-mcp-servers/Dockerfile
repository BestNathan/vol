# Build: docker build --build-arg BIN_NAME=docs-rs-mcp \
#   -t crpi-ck06yio90i1ttwlz.cn-beijing.personal.cr.aliyuncs.com/n_common/vol-monitor:docs-rs-mcp \
#   -f crates/vol-mcp-servers/Dockerfile .
#
# Multi-stage Alpine build:
#   Stage 1: Compile binary with Rust (rsproxy mirror)
#   Stage 2: Minimal Alpine runtime (~30MB)

# ── Stage 1: Builder ──────────────────────────────────────────────────────
FROM alpine:3.21 AS builder

ENV RUSTUP_DIST_SERVER=https://rsproxy.cn \
    RUSTUP_UPDATE_ROOT=https://rsproxy.cn/rustup \
    RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    HOME=/root \
    PATH=/usr/local/cargo/bin:$PATH

# Install apk deps via Alibaba mirror, then Rust via rsproxy
RUN sed -i 's/dl-cdn.alpinelinux.org/mirrors.aliyun.com/g' /etc/apk/repositories && \
    apk add --no-cache jq curl gcc g++ musl-dev pkgconfig openssl-dev && \
    curl --proto '=https' --tlsv1.2 -sSf https://rsproxy.cn/rustup-init.sh | sh -s -- -y

# Copy cargo mirror configuration
COPY .cargo/ .cargo/

WORKDIR /app

# ── Dependency cache layer ────────────────────────────────────────────────
# Only Cargo.toml. This layer is cached until dependencies change.
COPY crates/vol-mcp-servers/Cargo.toml Cargo.toml

# Create stub source files so cargo can resolve deps without real source code.
RUN cargo read-manifest | jq -r '.targets[] | select(.kind[] == "bin") | .src_path' | while read src_path; do \
        mkdir -p "$(dirname "$src_path")"; \
        echo "fn main() {}" > "$src_path"; \
    done && \
    echo "// lib" > src/lib.rs 2>/dev/null || true && \
    cargo check --all-targets

# ── Full source copy + real build ─────────────────────────────────────────
COPY crates/vol-mcp-servers .

ARG BIN_NAME=__REQUIRED__
RUN cargo build --release --bin ${BIN_NAME}

# ── Stage 2: Runtime ──────────────────────────────────────────────────────
FROM alpine:3.21

ARG BIN_NAME=__REQUIRED__
ENV BIN_NAME=${BIN_NAME}

COPY --from=builder /app/target/release/${BIN_NAME} /usr/local/bin/${BIN_NAME}

EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/${BIN_NAME}"]
CMD ["--http", "0.0.0.0:8080"]
