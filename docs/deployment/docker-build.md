# Docker Build Guide

## Prerequisites

### Cargo Registry Mirror (China)

The project uses rsproxy.cn mirror for crates.io. Ensure `.cargo/config.toml` exists:

```toml
[source.crates-io]
replace-with = 'rsproxy-sparse'
[source.rsproxy]
registry = "https://rsproxy.cn/crates.io-index"
[source.rsproxy-sparse]
registry = "sparse+https://rsproxy.cn/index/"
[registries.rsproxy]
index = "https://rsproxy.cn/crates.io-index"
[net]
git-fetch-with-cli = true
```

## Dockerfile (Multi-stage Build)

```dockerfile
# Stage 1: Build
FROM rust:latest AS builder

WORKDIR /app

# Copy cargo config for registry mirror
COPY .cargo ./.cargo

# Copy dependency definitions first for better caching
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/

# Build release binary
RUN cargo build --release -vv

# Stage 2: Runtime
FROM debian:bookworm-slim

# Install CA certificates using Aliyun mirror
RUN sed -i 's|deb.debian.org|mirrors.aliyun.com|g' /etc/apt/sources.list.d/debian.sources && \
    apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

# Copy the binary from builder
COPY --from=builder /app/target/release/vol-monitor /usr/local/bin/vol-monitor

WORKDIR /app

# Run the binary
ENTRYPOINT ["/usr/local/bin/vol-monitor"]
```

**Key points:**
- `.cargo/config.toml` must be copied into the image for rsproxy mirror to work
- Using sparse registry protocol for faster dependency resolution
- Aliyun mirror for apt packages (`deb.debian.org` → `mirrors.aliyun.com`)
- Multi-stage build keeps final image ~95MB

## Build Commands

### Single Architecture (Current Platform)

```bash
# Build image
docker build -t crpi-ck06yio90i1ttwlz.cn-beijing.personal.cr.aliyuncs.com/n_common/vol-monitor:latest .

# Push to ACR
docker push crpi-ck06yio90i1ttwlz.cn-beijing.personal.cr.aliyuncs.com/n_common/vol-monitor:latest
```

### Multi-Architecture (amd64 + arm64)

**Setup (one-time):**

```bash
# Create multi-arch builder (requires Docker buildx)
docker buildx create --use --name multiarch --driver docker-container
docker buildx inspect multiarch --bootstrap
```

**Build and push:**

```bash
docker buildx build --platform linux/amd64,linux/arm64 \
    --push -t crpi-ck06yio90i1ttwlz.cn-beijing.personal.cr.aliyuncs.com/n_common/vol-monitor:latest .
```

**Verify:**

```bash
docker buildx imagetools inspect crpi-ck06yio90i1ttwlz.cn-beijing.personal.cr.aliyuncs.com/n_common/vol-monitor:latest
# Output should show:
#   Manifests:
#     ...  # linux/amd64
#     ...  # linux/arm64
```

**Notes:**
- First build may take 5-10 minutes due to QEMU emulation for arm64
- `--push` is required (multi-arch images cannot be loaded locally)
- The resulting image is a manifest list containing both architectures
- Kubernetes will automatically pull the correct architecture for each node

## Deploy to Kubernetes

```bash
# Deploy to k8s
kubectl apply -f k8s/namespace.yaml
kubectl apply -f k8s/configmap.yaml
kubectl apply -f k8s/deployment.yaml
```

Or use the one-click deploy script:
```bash
./k8s/deploy.sh latest
```
