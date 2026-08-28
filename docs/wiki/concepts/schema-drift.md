---
type: concept
category: pattern
tags: [schema, config, drift, failure-mode, debugging, serde, toml]
created: 2026-08-28
updated: 2026-08-28
source_count: 1
---

# Schema Drift

## Definition

**Schema drift** is the failure mode where a configuration format and the code that parses it diverge — one is migrated to a new schema while the other is not. The system keeps starting successfully, but the drifted component silently produces nothing.

Drift is distinct from an ordinary parse bug in two ways: the config file is usually *correct* (it is the newer artifact), and the failure surfaces far from its cause.

## Why it hides

Drift is only dangerous when combined with **warn-and-skip error handling**. Config loaders are commonly written to tolerate individual bad entries so one malformed file cannot take down a service:

```text
for each config file:
    parse → on error: tracing::warn!(...) ; continue
```

That is correct for isolated corruption and wrong for systemic drift. When *every* file fails the same way, warn-and-skip converts a total failure into a clean startup with an empty registry. Downstream consumers then do their own warn-and-skip on the missing entries, and the failure is two hops from its cause with no error return anywhere in the chain.

## Diagnostic signature

Suspect drift, not a config typo, when:

1. **A whole class of things is missing, not one.** Zero tools registered, not one broken tool.
2. **The config files look right** when read against the current type definition's *intent*, and match a recent commit's format.
3. **`git log` on the config directory shows a migration commit** that did not touch the parser. Check whether the commit that changed the config schema also changed the loader.
4. **Two parallel implementations exist** for the same responsibility — a common precursor, since migrating one leaves the other stale.

The fastest confirmation is to extract the parser type into a standalone harness and feed it the production config verbatim. This separates "does this file parse" from all downstream behavior.

## Example: sandbox config drift (2026-08-27 → 2026-08-28)

Commit `11a28f09` rewrote all four `.agents/sandboxes/*.toml` files to the new `SandboxSpec` format:

```toml
# after 11a28f09
name = "ansible-prod"
provider = "ssh"          # was: type = "ssh"
host = "192.168.2.106"    # was: nested under [ssh]
key_path = "/app/..."     # was: identity_file
```

The commit message documented the intent precisely — *"Change 'type' to 'provider', flatten SSH config, rename identity_file to key_path"* — but touched only the TOML files. `SandboxRegistry::load()`, still the loader used by `AgentRuntime` and `cli-tools-mcp`, went on deserializing the old `SandboxConfig` struct with `#[serde(rename = "type")]`.

Result: all four sandboxes failed with `missing field 'type'` → warn+skip. All three cli-tools then failed their `sandbox_ref` lookup → warn+skip. `cli-tools-mcp` served zero tools while reporting healthy.

The precursor was two parallel systems: `SandboxRegistry` (data-plane) and `SandboxManager` (control-plane). The TOML migration was written against the `SandboxManager` schema; the data-plane path was never updated. See [[sandbox-registry-manager-unification]].

## Prevention

- **Migrate config and parser in the same commit.** A schema change that touches only one side is incomplete by construction.
- **Do not maintain two loaders for one config format.** Parallel implementations guarantee that a future migration updates only one. Consolidate before migrating.
- **Assert on expected-vs-actual counts at startup.** "3 cli-tools configured, 0 registered" is a loud, specific signal that warn-and-skip cannot produce on its own. Cheap to add, and it converts silent drift into an immediate error.
- **Test the parser against real config files.** Unit tests written alongside a new schema naturally use the new format. A test that reads the actual `.agents/` files, or a fixture copied verbatim from production, catches drift that hand-written test strings cannot.
- **Prefer serde aliases over hard renames** when a field is renamed (`#[serde(alias = "...")]`), so both spellings parse during the migration window.

## Related Concepts

- [[sandbox-lifecycle]] — the subsystem where this instance of drift occurred
- [[cli-style-tool-pattern]] — the downstream consumer whose tools went missing
- [[lifecycle-state-machine]] — sibling concept in the same subsystem
