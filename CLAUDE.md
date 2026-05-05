# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

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
