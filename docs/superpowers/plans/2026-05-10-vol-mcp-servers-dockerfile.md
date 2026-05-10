# vol-mcp-servers Dockerfile Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Dockerfile and .dockerignore to build minimal Alpine images for any MCP server binary in vol-mcp-servers.

**Architecture:** Two-stage multi-stage Dockerfile at the workspace root. Builder stage installs Alpine Rust toolchain and compiles the binary specified by `ARG BIN_NAME`. Runtime stage copies the stripped binary into a clean Alpine image with only `ca-certificates`.

**Tech Stack:** Docker multi-stage build, Alpine 3.21, cargo, strip.

---

### Task 1: Create Dockerfile

**Files:**
- Create: `crates/vol-mcp-servers/Dockerfile`

- [ ] **Step 1: Create the Dockerfile**

```dockerfile
FROM alpine:3.21 AS builder
ARG BIN_NAME

RUN apk add --no-cache rust cargo gcc libc-dev
WORKDIR /src

COPY Cargo.toml Cargo.lock ./
COPY crates/vol-mcp-servers/Cargo.toml crates/vol-mcp-servers/
RUN cargo fetch --locked

COPY crates/vol-mcp-servers/src crates/vol-mcp-servers/src
RUN cargo build --release --bin ${BIN_NAME} -p vol-mcp-servers
RUN strip target/release/${BIN_NAME}

FROM alpine:3.21
ARG BIN_NAME

RUN apk add --no-cache ca-certificates

COPY --from=builder /target/release/${BIN_NAME} /usr/local/bin/${BIN_NAME}

EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/docs-rs-mcp"]
CMD ["--http", "0.0.0.0:8080"]
```

Note: The `ENTRYPOINT` uses the default binary `docs-rs-mcp`. When building with `--build-arg BIN_NAME=future-mcp`, the builder stage compiles `future-mcp` and copies it, but the runtime ENTRYPOINT string remains `docs-rs-mcp` because Docker ARG does not propagate to ENTRYPOINT at runtime. To fix this, we need to use a shell form or a wrapper script. However, since each image is built for a specific binary, we can use the ARG trick with ENV:

Actually, the correct approach for runtime ARG in ENTRYPOINT is to use ENV or a shell wrapper. Let's use the simpler approach: set ENV at the end of the Dockerfile:

```dockerfile
FROM alpine:3.21 AS builder
ARG BIN_NAME

RUN apk add --no-cache rust cargo gcc libc-dev
WORKDIR /src

COPY Cargo.toml Cargo.lock ./
COPY crates/vol-mcp-servers/Cargo.toml crates/vol-mcp-servers/
RUN cargo fetch --locked

COPY crates/vol-mcp-servers/src crates/vol-mcp-servers/src
RUN cargo build --release --bin ${BIN_NAME} -p vol-mcp-servers
RUN strip target/release/${BIN_NAME}

FROM alpine:3.21
ARG BIN_NAME

RUN apk add --no-cache ca-certificates

ENV BIN_NAME=${BIN_NAME}
COPY --from=builder /target/release/${BIN_NAME} /usr/local/bin/${BIN_NAME}

EXPOSE 8080
ENTRYPOINT ["/bin/sh", "-c", "/usr/local/bin/${BIN_NAME}"]
CMD ["--http", "0.0.0.0:8080"]
```

This way, `BIN_NAME` is set as ENV (persisted at runtime), and ENTRYPOINT uses `/bin/sh -c` to expand the variable.

- [ ] **Step 2: Verify with dry-run docker build**

```bash
docker build --build-arg BIN_NAME=docs-rs-mcp -f crates/vol-mcp-servers/Dockerfile . --no-cache
```

Expected: Build succeeds, produces an image.

- [ ] **Step 3: Verify image size**

```bash
docker images | grep vol-monitor
```

Expected: Image size ~30-50MB.

- [ ] **Step 4: Verify container starts**

```bash
docker run --rm crpi-ck06yio90i1ttwlz.cn-beijing.personal.cr.aliyuncs.com/n_common/vol-monitor:docs-rs-mcp --help 2>&1 || docker run --rm <local-image-id> --help
```

Expected: clap help output showing `--http` option.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-mcp-servers/Dockerfile
git commit -m "feat: add Dockerfile for vol-mcp-servers Alpine multi-stage build"
```

---

### Task 2: Create .dockerignore

**Files:**
- Create: `crates/vol-mcp-servers/.dockerignore`

- [ ] **Step 1: Create the .dockerignore**

Place it at the workspace root since Docker build context is the workspace root:

```
.git/
target/
.claude/
docs/
README.md
!.dockerignore
```

This is simpler than the whitelist approach — we exclude the big directories (git, build artifacts, docs, IDE) and let everything else through. The Cargo.toml, Cargo.lock, and crates/ directory are all needed for compilation.

- [ ] **Step 2: Commit**

```bash
git add .dockerignore
git commit -m "chore: add .dockerignore for vol-mcp-servers docker build"
```

---

### Task 3: Test full build-and-run cycle

- [ ] **Step 1: Build image with tag**

```bash
docker build --build-arg BIN_NAME=docs-rs-mcp \
  -t crpi-ck06yio90i1ttwlz.cn-beijing.personal.cr.aliyuncs.com/n_common/vol-monitor:docs-rs-mcp \
  -f crates/vol-mcp-servers/Dockerfile .
```

Expected: Build completes successfully.

- [ ] **Step 2: Run container and verify startup**

```bash
timeout 3 docker run --rm \
  crpi-ck06yio90i1ttwlz.cn-beijing.personal.cr.aliyuncs.com/n_common/vol-monitor:docs-rs-mcp \
  2>&1 || true
```

Expected: Logs showing "docs-rs-mcp running on stdio" then exits (no stdin in docker).

- [ ] **Step 3: Run container with HTTP and test**

```bash
docker run -d --name test-mcp -p 9999:8080 \
  crpi-ck06yio90i1ttwlz.cn-beijing.personal.cr.aliyuncs.com/n_common/vol-monitor:docs-rs-mcp
sleep 2
curl -s http://127.0.0.1:9999/ \
  -H "Content-Type: application/json" \
  -H "Accept: application/json, text/event-stream" \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0.1"}}}'
docker stop test-mcp && docker rm test-mcp
```

Expected: MCP initialize response with `{"capabilities":{"tools":{}}}`.

- [ ] **Step 4: Check image size**

```bash
docker images | grep "vol-monitor"
```

Expected: ~30-50MB.

- [ ] **Step 5: Commit (no code changes, just verification)**

No commit needed — this is verification only.
