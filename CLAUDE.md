# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with this repository.

## Docker + Rust Build Configuration

All Docker-based Rust builds must use rsproxy as the mirror source. The build environment
cannot access crates.io directly.

### Environment Variables (Dockerfile builder stage)

```dockerfile
ENV RUSTUP_DIST_SERVER=https://rsproxy.cn \
    RUSTUP_UPDATE_ROOT=https://rsproxy.cn/rustup \
    RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH
```

### Rust Installation

```dockerfile
RUN curl --proto '=https' --tlsv1.2 -sSf https://rsproxy.cn/rustup-init.sh | sh -s -- -y
```

### Cargo Mirror Config (`.cargo/config.toml`)

Must be copied into the builder stage. Contains:
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

## Conventions

- When finished a devlopment task, you **MUST** use skill `wiki-ingest` to add or update project wiki at `docs/wiki`

- When `docs/superpowers/*` add or update docs you **MUST** upload the doc to lark wiki space **7630485291026910436**
```bash
# create wiki doc
lark-cli docs +create \
    --title "{title}" \
    --markdown "$(cat path/to/markdown.md)" \
    --wiki-space "{wiki space id}" \
    --as user
```
