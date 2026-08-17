---
type: entity
category: product
tags: [crate, context, prompt, contributor, rust]
created: 2026-08-17
updated: 2026-08-17
source_count: 1
---

# vol-llm-context Crate

**Category:** Rust crate — context/prompt construction
**Related:** [[vol-llm-core-crate]], [[vol-llm-agent-crate]], [[context-builder]], [[session-contributor]], [[session-as-ssot]]

## Overview

Implements pluggable prompt construction: the `ContextBuilder` assembles system prompt, session history, skill context, and user input into a single message list via `ContextContributor` implementations, with token-budgeted compression and per-contributor snapshots.

## Key Facts

- `ContextBuilder`: assembles contributor output into a message list; `build()` applies compress paths when the estimated size exceeds the token budget (dropping lowest-priority zones first)
- `ContextContributor` trait: `contribute()` → `Result<Vec<ContextBlock>, ContextError>`; `replace_contributor`, `contributor_names`, `contributor_infos`, `add_contributors_from`, `snapshot_by_name` (all roles, None content handling)
- Builtin contributors: `builtin::simple`, `builtin::file` (`FileContributor`), `builtin::user_input` (`UserInputContributor`)
- Token estimation with per-image budget; `token_budget` / budget drop loop across zones
- `ContextError`: error type for context building failures [[context-error]]

## Timeline

- **2026-05-04**: Initial context builder design ingested with the session-as-SSoT redesign [[session-ssot-redesign]]
- **2026-08-17**: Coverage raised to ≥80% (test-only, commit `f6edf792`): 65.19% → 88.94% regions / 90.08% lines, `just cover-gate vol-llm-context 80` PASS; +19 tests covering `replace_contributor`, `token_budget`, contributor listing APIs, `snapshot_by_name` error paths, `build()` compress path, middle-zone budget drop loop, `Clone` impls, and first-ever full test modules for `builtin::simple` (previously 0% covered), `FileContributor`, `UserInputContributor`, and the `ContextContributor` test-double [[coverage-gate-work]]
