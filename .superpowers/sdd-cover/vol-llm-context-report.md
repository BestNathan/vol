# vol-llm-context coverage report (target ≥ 80%)

## Result

`just cover-gate vol-llm-context 80` now PASSES.

```
$ just cover-gate vol-llm-context 80
PASS: vol-llm-context line coverage is 88.94% (≥ 80%)
```

Note: the gate parses column $4 of the llvm-cov TOTAL row, which is **region** coverage (the
task brief said line coverage; both improved far beyond 80%):
- Before: 65.19% regions / 67.41% lines (the "78.10%" figure in the brief was stale — actual
  baseline measured on this checkout was 65.19%)
- After: 88.94% regions / 90.08% lines

## Per-file breakdown

| File | Before (regions) | After (regions) | After (lines) | What was uncovered / what was added |
|---|---|---|---|---|
| builder.rs | 50.75% (295/599 missed) | 82.66% (194/1119) | 84.13% | `replace_contributor` (both branches), `token_budget()`, `contributor_names()`, `contributor_infos()` (head/middle/tail/unknown zones, error propagation), `snapshot_by_name` (assistant/tool roles, `content: None` rendering, not-found error, contributor-error propagation), `build()` compress path, middle-zone drop loop, error propagation, `Clone` for `ContextBuilder` and `ContextBuilderBuilder`, `add_contributors_from` |
| builtin/file.rs | 87.64% | 98.41% | 100.00% | `name()`, `estimate_size()` (missing-file skip), `clone_box()` |
| builtin/simple.rs | 0.00% (no tests at all) | 99.31% | 100.00% | Full test module: `new`, `system`, `name`, `contribute`, `compress` (no-op), `estimate_size`, `clone_box` |
| builtin/user_input.rs | 81.36% | 98.98% | 100.00% | `name()`, `compress` (no-op), `estimate_size()` |
| context_block.rs | 95.91% | 95.91% | 96.85% | 4 lines at the `IMAGE_TOKEN_BUDGET` const (doc comment + `pub const` decl) have no runtime counter and cannot be covered; no code change possible/needed |
| context_contributor.rs | 77.27% | 100.00% | 100.00% | `name()`, `estimate_size()`, `clone_box()` of the test contributor; strengthened `compress_then_contribute` to assert truncated content |

## Tests added

- **19 new tests** (47 total now, previously 19 passed — 28 new/strengthened test fns; 19 new
  production-behavior tests plus 3 helper contributor structs: `CompressingContributor`,
  `FailingContributor`, `EmptyContributor` in builder.rs tests, plus 1 strengthened existing
  assertion in context_contributor.rs).
- Files touched (all test-only additions, 0 production lines modified):
  - `crates/vol-llm-context/src/builder.rs` (+426)
  - `crates/vol-llm-context/src/builtin/simple.rs` (+79, new `#[cfg(test)]` mod — file was 0% covered)
  - `crates/vol-llm-context/src/builtin/file.rs` (+47)
  - `crates/vol-llm-context/src/builtin/user_input.rs` (+25)
  - `crates/vol-llm-context/src/context_contributor.rs` (+37)

Key behavioral tests (non-vacuous assertions):
- `test_build_compresses_contributors_when_over_budget` — asserts compressed head content ("xxxxx")
  and unchanged no-op tail after `build()` takes the compress path.
- `test_build_drops_lowest_priority_middle_when_over_budget` — 4 middle blocks vs 800-token middle
  budget: asserts exactly the last (highest-position) middle block is dropped and head/tail order.
- `test_snapshot_by_name_all_roles_and_none_content` — asserts system/assistant/tool role strings
  and empty-string rendering for `content: None`.
- `test_contributor_infos_zone_labels_and_counts` — asserts head/middle/tail/"unknown" zone labels
  and message counts, incl. `estimated_tokens` values.
- `test_replace_contributor_*`, `test_context_builder_clone_*`, `test_builder_builder_clone_*`,
  `test_snapshot_by_name_not_found`, and the three error-propagation tests (build / infos /
  snapshot with a failing contributor) all assert concrete outcomes.

## Cargo test result

```
$ cargo test -p vol-llm-context
test result: ok. 47 passed; 0 failed; 0 ignored
   Doc-tests vol_llm_context: running 0 tests
```

## Production-bug concerns

None found. One behavioral observation (not a bug): `contributor_infos()` reports
`anchor_zone: "unknown"` only for contributors whose `contribute()` returns zero blocks;
a contributor returning a block with zero messages still reports its zone (the "unknown" zone
test required a dedicated `EmptyContributor` test double — the existing `SimpleContributor`
always emits one block). Also, `ContextOutput` lacks a `Debug` derive, so tests must match on
`build()` errors instead of `unwrap_err()` — cosmetic, not a bug.
