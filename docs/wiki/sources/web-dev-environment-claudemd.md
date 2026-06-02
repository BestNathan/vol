---
type: source
source_type: report
date: 2026-05-27
ingested: 2026-05-27
tags: [web, dioxus, tailwind, claude-md, environment]
---

# CLAUDE.md Web Development Environment Update

**Authors/Creators:** Claude Code session
**Date:** 2026-05-27
**Link:** `CLAUDE.md`

## TL;DR

`CLAUDE.md` now documents the web-only development prerequisites for `vol-llm-ui`: Rust/Cargo, the WASM target, Dioxus CLI 0.6.x, `cargo-watch`, Node/npm, and the `crates/vol-llm-ui` npm dependencies. It also records the correct Tailwind watch command and a Dioxus `--platform web` fallback for the 404 dev-server symptom.

## Key Takeaways

- Web development requires installing `dx` with `cargo install dioxus-cli --version 0.6.3 --locked`.
- Backend auto-reload requires `cargo-watch`.
- Tailwind v4 CSS compilation requires `npm ci --prefix crates/vol-llm-ui` before running the CLI.
- `make web-css` now runs Tailwind in persistent watch mode using `--watch=always`.
- `.claude/skills/vol-web-dev/SKILL.md` is tracked as the project-specific web development run/debug guide.
- If `make web-dev` serves `Err 404 - dioxus is not currently serving a web app`, use `dx serve --platform web ...` explicitly.

## Detailed Summary

The project guidance in `CLAUDE.md` was updated under the Web Frontend section to make the setup requirements explicit. The new table covers command-line tools and install/verification commands needed before starting the Dioxus WASM frontend and JSON-RPC backend.

The startup section now makes `make web-css` the canonical persistent Tailwind watch command:

```bash
npx --prefix crates/vol-llm-ui @tailwindcss/cli \
  -i crates/vol-llm-ui/assets/input.css \
  -o crates/vol-llm-ui/assets/tailwind.css \
  --watch=always
```

This reflects the observed behavior that Tailwind CLI exits after a one-time build unless explicit watch mode is used, and `--watch=always` keeps watching when launched from a background non-interactive process. The project skill `.claude/skills/vol-web-dev/SKILL.md` now captures the same three-service startup workflow for future Claude Code sessions.

The document also keeps the Makefile-driven workflow for `make web-dev`, `make web-backend`, `make web-check`, `make web-build`, and `make web-clippy`, while documenting an explicit fallback command when Dioxus CLI starts a server but does not serve the web app:

```bash
dx serve --platform web --package vol-llm-ui --bin vol-llm-ui-web \
  --no-default-features --features web --addr 0.0.0.0 --port 8080
```

## Entities Mentioned

- [[vol-llm-ui-crate]]: Dioxus WASM frontend requiring the documented web development toolchain.
- [[vol-llm-agent-channel-crate]]: JSON-RPC backend service started by `make web-backend`.

## Concepts Covered

- [[dioxus-web-pattern]]: Web app development now includes explicit CLI/platform requirements.
- [[tailwind-css-migration]]: Tailwind v4 workflow now documents npm dependency installation and persistent watch mode.

## Notes

The update includes the Makefile target alignment, generated Tailwind CSS output, and the tracked project skill so the documented web startup workflow is executable from the repository.
