# Build: docker build --build-arg BIN_NAME=docs-rs-mcp \
#   -t crpi-ck06yio90i1ttwlz.cn-beijing.personal.cr.aliyuncs.com/n_common/vol-monitor:docs-rs-mcp \
#   -f crates/vol-mcp-servers/Dockerfile .
#
# Multi-stage Alpine build:
#   Stage 1: Compile binary with Rust (using rsproxy mirror via .cargo/config.toml)
#   Stage 2: Minimal Alpine runtime (~30MB)
#
# The ENV BIN_NAME + ENTRYPOINT shell pattern allows the same Dockerfile to
# work with any binary name passed via --build-arg.

# ── Stage 1: Builder ──────────────────────────────────────────────────────
FROM alpine:3.21 AS builder

# Use Alibaba Cloud mirrors for Alpine packages
RUN sed -i 's/dl-cdn.alpinelinux.org/mirrors.aliyun.com/g' /etc/apk/repositories

RUN apk add --no-cache curl gcc g++ musl-dev pkgconfig openssl-dev

# Install Rust via rustup (default target is x86_64-unknown-linux-musl on Alpine)
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

# Copy cargo mirror configuration
COPY .cargo/ /root/.cargo/

# Set working directory and copy Cargo manifests for dependency caching
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates/vol-mcp-servers/Cargo.toml crates/vol-mcp-servers/

# Fetch dependencies (cached layer unless Cargo.toml/lock changes)
RUN mkdir -p crates/vol-mcp-servers/src && \
    echo 'fn main() {}' > crates/vol-mcp-servers/src/lib.rs && \
    mkdir -p crates/vol-mcp-servers/src/docs_rs && \
    touch crates/vol-mcp-servers/src/docs_rs/mod.rs && \
    mkdir -p crates/vol-mcp-servers/src/transport && \
    touch crates/vol-mcp-servers/src/transport/mod.rs && \
    mkdir -p crates/vol-mcp-servers/src/bin && \
    touch crates/vol-mcp-servers/src/bin/docs_rs.rs && \
    cargo build --release --bin docs-rs-mcp -p vol-mcp-servers || true && \
    rm -rf crates/vol-mcp-servers/src

# Copy full source and build
COPY . .
ARG BIN_NAME=docs-rs-mcp
RUN cargo build --release --bin ${BIN_NAME} -p vol-mcp-servers

# ── Stage 2: Runtime ──────────────────────────────────────────────────────
FROM alpine:3.21

# Use Alibaba Cloud mirrors for runtime packages (if any needed)
RUN sed -i 's/dl-cdn.alpinelinux.org/mirrors.aliyun.com/g' /etc/apk/repositories

ARG BIN_NAME=docs-rs-mcp

# Make the binary name available at runtime (ARG does not persist)
ENV BIN_NAME=${BIN_NAME}

COPY --from=builder /app/target/release/${BIN_NAME} /usr/local/bin/${BIN_NAME}

EXPOSE 8080
ENTRYPOINT ["/bin/sh", "-c", "/usr/local/bin/${BIN_NAME} \"$@\"", "--"]
CMD ["--http", "0.0.0.0:8080"]
