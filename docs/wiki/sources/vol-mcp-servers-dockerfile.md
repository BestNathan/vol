---
type: source
category: implementation
tags: [docker, mcp, vol-mcp-servers, alpine, multi-stage]
created: 2026-05-10
updated: 2026-05-10
---

# vol-mcp-servers Dockerfile Implementation

**Category:** Implementation
**Related:** [[vol-mcp-servers-crate]], [[docs-rs-mcp-impl]]

## Overview

Added multi-stage Alpine Dockerfile to package vol-mcp-servers binaries into minimal (~30MB) Docker images for deployment to ACR (Alibaba Cloud Container Registry).

## Key Decisions

### Multi-Stage Alpine Build

Two-stage build:
1. **Builder**: `alpine:3.21` with Rust toolchain, compiles binary from source
2. **Runtime**: Minimal `alpine:3.21` with only the compiled binary

### China Mirror Configuration

Two mirror sources configured to handle network restrictions:

1. **Alpine packages (apk)**: `mirrors.aliyun.com` replaces `dl-cdn.alpinelinux.org` in `/etc/apk/repositories`
2. **Rust crates**: `.cargo/config.toml` copied into builder, which contains rsproxy.cn mirror for crates.io

```toml
# .cargo/config.toml
[source.crates-io]
replace-with = 'rsproxy-sparse'
[source.rsproxy-sparse]
registry = "sparse+https://rsproxy.cn/index/"
```

### ENTRYPOINT Pattern

```dockerfile
ENV BIN_NAME=${BIN_NAME}
ENTRYPOINT ["/bin/sh", "-c", "/usr/local/bin/${BIN_NAME} \"$@\"", "--"]
CMD ["--http", "0.0.0.0:8080"]
```

- `ENV` persists ARG value at runtime (Docker ARG does not)
- `"$@"` captures CMD arguments (the `"--"` is `$0`)
- Allows `docker run <image> --help` to work correctly

### Image Registry

Images are built and pushed to GHCR by the `build-mcp-images.yml` workflow as
`ghcr.io/bestnathan/<service>:<git-sha>` (e.g. `ghcr.io/bestnathan/docs-rs-mcp:46eea58`).
The earlier ACR path `.../n_common/<pipeline-monitor>:<binary-name>` is superseded — the
pipeline monitor binary was removed with the volatility pipeline on 2026-08-21.

## Files

- `crates/vol-mcp-servers/Dockerfile` — multi-stage Alpine packaging
- `.dockerignore` — excludes .git, target (except release), docs, k8s
- `.cargo/config.toml` — rsproxy mirror configuration (copied into builder)

## Build Command

```bash
docker build --build-arg BIN_NAME=docs-rs-mcp \
  -t docs-rs-mcp:docs-rs-mcp \
  -f crates/vol-mcp-servers/Dockerfile .
```

`BIN_NAME` is required — omitting it causes the build to fail immediately (`__REQUIRED__` default).
