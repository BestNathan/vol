---
type: concept
category: technique
tags: [serde, migration, task-id, backward-compatibility]
created: 2026-09-02
updated: 2026-09-02
source_count: 1
---

# Lenient Serde for Zero-Migration Type Changes

**Category:** Backward-compatible serialization pattern
**Related:** [[vol-llm-task-crate]], [[session-as-ssot]], [[vol-session]]

## Definition

When a type's canonical serialized form changes, hand-write `Deserialize` to accept **all previous forms plus the new one**, but have `Serialize` write only the new form. Old rows keep loading without a migration; new rows use the new form. The change is invisible to callers that just read and write — the asymmetry is localized to the type's serde impls.

## Key Facts

- The canonical change lives in **one place**: the type's serde impls. No migration, no schema change, no fixture rewrites.
- `Serialize` writes only the new canonical form. There is exactly one way to write a value; readers see consistent output.
- `Deserialize` accepts every form the type has ever taken — the old form, the new form, and any intermediate or historical forms that were ever written.
- A `FromStr` implementation mirrors `Deserialize` for string-entry points (CLI arguments, configuration).
- **Guard tests prove the zero-migration claim**: serialize a value through the old code (or hand-write a legacy fixture), deserialize through the new code, assert equality. If the guard fails, a migration is required.

## How It Works

```rust
// Before: derive
#[derive(Serialize, Deserialize)]
pub struct TaskId(pub u64);
impl Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "t{}", self.0) }
}

// After: hand-written, lenient on read, canonical on write
impl Serialize for TaskId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer {
        serializer.serialize_str(&self.0.to_string())  // always "1"
    }
}

impl<'de> Deserialize<'de> for TaskId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: Deserializer<'de> {
        struct V;
        impl Visitor<'_> for V {
            // ...
            fn visit_u64<E>(self, v: u64) -> Result<TaskId, E> { Ok(TaskId(v)) }
            fn visit_i64<E>(self, v: i64) -> Result<TaskId, E> {
                u64::try_from(v).map(TaskId).map_err(|_| E::custom("negative"))
            }
            fn visit_str<E>(self, v: &str) -> Result<TaskId, E> {
                TaskId::from_str(v).map_err(E::custom)  // strips one 't'
            }
        }
        deserializer.deserialize_any(V)  // self-describing required
    }
}
```

Readers see three input forms (`1`, `"1"`, `"t1"`), writers produce one (`"1"`). A database row with `"id": 1` from the old code keeps loading; a new row writes `"id": "1"`.

## Constraints

- **Self-describing format required.** `deserialize_any` only works for JSON, RON, etc. — not bincode, postcard, or anything that is not self-describing. Verify no non-self-describing serializer is in the dependency graph.
- **The type must be the only thing reading the data.** If a human writes JSON by hand, or a different system writes the data, lenient deserialization may silently accept malformed input that would otherwise error. For internal data, this is fine; for external APIs, the trade-off is different.
- **Guard tests are load-bearing.** Without tests that round-trip legacy-shaped data through the new code, a regression in the lenient path can go unnoticed. `TaskId` has explicit tests for bare integers, canonical strings, single-`t`-prefixed strings, and negative cases.

## Examples / Applications

### TaskId unification

`TaskId` changed from bare-integer JSON to canonical-string JSON. `Deserialize` accepts all three forms, `Serialize` writes only the string. Old rows in:
- `DatabaseTaskStore` (`dependencies_json`, `blocks_json` columns — JSON arrays)
- `FileTaskStore` (`{id}.json` body — `"id": 1`)

keep loading. The zero-migration claim is guarded by `test_file_store_reads_legacy_numeric_ids` and `test_database_store_reads_legacy_dependencies_json`, which construct fixtures in the old shape and assert they deserialize correctly through the new code.

### The `FromStr` mirror

`TaskId::from_str` accepts the same forms as `Deserialize` and rejects the same malformed input. Used by `parse_task_id_arg` in the CLI so `--id 1`, `--id t1`, and `--id "1"` all work.

## Pitfalls Observed

1. **Do not use `trim_start_matches('t')` to accept the prefix.** It accepts `"ttt1"`. Use `strip_prefix('t').unwrap_or(s)` which strips at most one.
2. **Do not put the leniency in a `TryFrom<String>` impl** if you also want `FromStr` — the two can drift. Implement one in terms of the other (`FromStr::from_str` calls a shared helper; `Deserialize::visit_str` calls `FromStr::from_str`).
3. **Test the failure direction.** A lenient `Deserialize` can pass tests spuriously but should not fail them when the implementation is correct. Verify by temporarily regressing to strict deserialization and confirming the legacy tests fail.
4. **`deserialize_any` is the only way** to accept heterogeneous input (number or string). `deserialize_str` would reject the numeric form; `deserialize_u64` would reject the string form.

## Related Concepts

- [[session-task-binding]]: consumes `TaskId` and relies on the lenient `Deserialize` for any persisted bindings written before the type change
- [[vol-llm-task-crate]]: the crate that owns `TaskId`
- [[vol-session]]: uses the same pattern for session metadata (degrades malformed JSON to empty via `unwrap_or_default`)
