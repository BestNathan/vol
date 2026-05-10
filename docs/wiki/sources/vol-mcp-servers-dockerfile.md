---
type: source
category: implementation
tags: [docker, mcp, vol-mcp-servers, alpine, ubuntu]
created: 2026-05-10
updated: 2026-05-10
---

# vol-mcp-servers Dockerfile Implementation

**Category:** Implementation
**Related:** [[vol-mcp-servers-crate]], [[docs-rs-mcp-impl]]

## Overview

Added Dockerfile to package vol-mcp-servers binaries into minimal Docker images for deployment to ACR (Alibaba Cloud Container Registry).

## Key Decisions

### Single-Stage Ubuntu Build

Originally spec'd as two-stage Alpine multi-stage build. Implementation changed to single-stage Ubuntu due to:
1. **Network restrictions**: Docker build environment cannot reach crates.io or Alpine package repos, making multi-stage cargo build impossible
2. **glibc incompatibility**: Host-compiled binary is dynamically linked against glibc. Alpine uses musl libc, causing `not found` errors at runtime

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

Registry: `crpi-ck06yio90i1ttwlz.cn-beijing.personal.cr.aliyuncs.com/n_common/vol-monitor:<binary-name>`

## Files

- `crates/vol-mcp-servers/Dockerfile` — single-stage Ubuntu packaging
- `.dockerignore` — excludes .git, target (except release), docs, k8s

## Build Command

```bash
docker build --build-arg BIN_NAME=docs-rs-mcp \
  -t crpi-ck06yio90i1ttwlz.cn-beijing.personal.cr.aliyuncs.com/n_common/vol-monitor:docs-rs-mcp \
  -f crates/vol-mcp-servers/Dockerfile .
```
