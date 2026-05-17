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

## Development

### Web Frontend

**`vol-llm-ui`** (`crates/vol-llm-ui`) is the web frontend crate. It contains the Dioxus 0.6 WASM app with components, Tailwind CSS, and web-specific state management. All web-related code lives under this crate.

**When developing or running the web frontend**, you must use the web-related Makefile commands — do NOT run generic `cargo build` or `cargo run` commands as they will not compile the WASM binary or serve the frontend.

All web development commands use the Makefile. Run `make help` to see available commands:

| Command | Description |
|---------|-------------|
| `make web-css` | Build Tailwind CSS (watch mode) |
| `make web-dev` | Start Dioxus dev server (port 8080) |
| `make web-backend` | Start backend JSON-RPC agent service (port 3001) |
| `make web-check` | cargo check (web only) |
| `make web-build` | Build WASM binary |
| `make web-clippy` | cargo clippy (web only) |

**Starting web development requires running 3 commands in separate terminals:**

1. `make web-css` — compile Tailwind CSS in watch mode (must stay running)
2. `make web-dev` — start Dioxus dev server on port 8080
3. `make web-backend` — start backend JSON-RPC agent service on port 3001

**Important:** `make web-css` must be running before `make web-dev`, otherwise new Tailwind utility classes (e.g., arbitrary values like `w-[600px]`, `h-[70vh]`) won't be compiled into the CSS and won't take effect.

## Conventions

- When finished a development task, you **MUST** use skill `wiki-ingest` to add or update project wiki at `docs/wiki`

- When `docs/superpowers/*` add or update docs you **MUST** upload the doc to lark wiki space **7630485291026910436**
```bash
# create wiki doc
lark-cli docs +create \
    --title "{title}" \
    --markdown "$(cat path/to/markdown.md)" \
    --wiki-space "{wiki space id}" \
    --as user
```

## Feishu Docs

- When `superpowers` skill writing a doc into `docs/superpowers/*`, you **MUST** upload it to feishu docs with `lark-cli`
- `docs/superpowers/plans/*`: wiki node id is **TEkkw1W6niuBxQkcvswchOo5nhb**
- `docs/superpowers/requirement/*`: wiki node id is **PPDZw7LFqiFjMTkAXFocFoO6nce**
- `docs/superpowers/specs/*`: wiki node id is **Og7twpiPoi0Vbjk2EzvcqX92nsb**


```sh
# lark-cli to upload docs to feishu
lark-cli docs +create \
    --title "{title}" \
    --markdown "$(cat path/to/markdown.md)" \
    --wiki-node "{wiki node id}"

# lark-cli to update docs to feishu, the token is the last part of url
# e.g: https://my.feishu.cn/wiki/PPDZw7LFqiFjMTkAXFocFoO6nce => token=**PPDZw7LFqiFjMTkAXFocFoO6nce**
lark-cli docs +update \
    --new-title "{title}" \
    --mode overwrite \
    --markdown "$(cat path/to/markdown.md)" \
    --doc "{doc url or token}"
```