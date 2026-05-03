# Unified `.agents/` Directory Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rename all `.agents/` directory references to `.agents/` across code, tests, and docs, eliminating the inconsistency between `.agents/` (vol-llm-skill) and `.agents/` (vol-llm-wiki, vol-llm-yaml-agent).

**Architecture:** Simple path string replacements in doc comments, code literals, and test fixtures. No behavioral changes. Follow existing patterns: use `Edit` tool for precise string replacements, `cargo check` to verify compilation, `cargo test -p vol-llm-wiki -p vol-llm-yaml-agent` to verify tests pass.

**Tech Stack:** Rust, grep, cargo

---

### Task 1: Update vol-llm-wiki code references

**Files:**
- Modify: `crates/vol-llm-wiki/src/lib.rs:3`
- Modify: `crates/vol-llm-wiki/src/config.rs:23`
- Modify: `crates/vol-llm-wiki/src/loader.rs:29,57`
- Modify: `crates/vol-llm-wiki/src/injector.rs:21`

- [ ] **Step 1: Edit `lib.rs` doc comment**

Change the crate-level doc comment from `.agents/wikis/` to `.agents/wikis/`:

```rust
// In crates/vol-llm-wiki/src/lib.rs, line 3
// Change:
//! Wiki pages live in `.agents/wikis/` with progressive loading
// To:
//! Wiki pages live in `.agents/wikis/` with progressive loading
```

- [ ] **Step 2: Edit `config.rs` doc comment**

```rust
// In crates/vol-llm-wiki/src/config.rs, line 23
// Change:
    /// Wiki pages are stored in `{working_dir}/.agents/wikis/`.
// To:
    /// Wiki pages are stored in `{working_dir}/.agents/wikis/`.
```

- [ ] **Step 3: Edit `loader.rs` doc comment and path literal**

Two changes in one file:

```rust
// In crates/vol-llm-wiki/src/loader.rs, line 29
// Change:
/// Discovers wiki pages from `.agents/wikis/` directories.
// To:
/// Discovers wiki pages from `.agents/wikis/` directories.

// In crates/vol-llm-wiki/src/loader.rs, line 57
// Change:
            roots.push(wd.join(".agent").join("wikis"));
// To:
            roots.push(wd.join(".agents").join("wikis"));
```

- [ ] **Step 4: Edit `injector.rs` doc comment**

```rust
// In crates/vol-llm-wiki/src/injector.rs, line 21
// Change:
    /// Create a WikiInjector that loads wiki pages from `{working_dir}/.agents/wikis`.
// To:
    /// Create a WikiInjector that loads wiki pages from `{working_dir}/.agents/wikis`.
```

- [ ] **Step 5: Verify compilation**

```bash
cargo check -p vol-llm-wiki
```
Expected: no errors

---

### Task 2: Update vol-llm-wiki integration test

**Files:**
- Modify: `crates/vol-llm-wiki/tests/wiki_integration_test.rs:53`

- [ ] **Step 1: Edit test directory path**

```rust
// In crates/vol-llm-wiki/tests/wiki_integration_test.rs, line 53
// Change:
    let wiki_dir = temp_wiki.path().join(".agent").join("wikis");
// To:
    let wiki_dir = temp_wiki.path().join(".agents").join("wikis");
```

- [ ] **Step 2: Verify test compiles**

```bash
cargo check -p vol-llm-wiki --tests
```
Expected: no errors

---

### Task 3: Update vol-llm-yaml-agent code references

**Files:**
- Modify: `crates/vol-llm-yaml-agent/src/lib.rs:8`
- Modify: `crates/vol-llm-yaml-agent/src/config.rs:80-81`
- Modify: `crates/vol-llm-yaml-agent/src/discovery.rs:27,29`

- [ ] **Step 1: Edit `lib.rs` doc comment**

```rust
// In crates/vol-llm-yaml-agent/src/lib.rs, line 8
// Change:
//! let agent = YamlAgentBuilder::from_file(".agents/agents/coding.yaml")?
// To:
//! let agent = YamlAgentBuilder::from_file(".agents/agents/coding.yaml")?
```

- [ ] **Step 2: Edit `config.rs` YAML example**

```rust
// In crates/vol-llm-yaml-agent/src/config.rs, lines 80-81
// Change:
  - .agents/AGENT.md
  - .agents/INSTRUCTION.md
// To:
  - .agents/AGENT.md
  - .agents/INSTRUCTION.md
```

- [ ] **Step 3: Edit `discovery.rs` doc comment and path literal**

```rust
// In crates/vol-llm-yaml-agent/src/discovery.rs, line 27
// Change:
/// Discover agents from the standard `.agents/agents/` directory.
// To:
/// Discover agents from the standard `.agents/agents/` directory.

// In crates/vol-llm-yaml-agent/src/discovery.rs, line 29
// Change:
    let agents_dir = working_dir.join(".agent").join("agents");
// To:
    let agents_dir = working_dir.join(".agents").join("agents");
```

- [ ] **Step 4: Verify compilation**

```bash
cargo check -p vol-llm-yaml-agent
```
Expected: no errors

---

### Task 4: Update vol-llm-yaml-agent discovery test

**Files:**
- Modify: `crates/vol-llm-yaml-agent/src/discovery.rs:66`

- [ ] **Step 1: Edit test directory path**

```rust
// In crates/vol-llm-yaml-agent/src/discovery.rs, line 66
// Change:
        let agents_dir = temp.path().join(".agent").join("agents");
// To:
        let agents_dir = temp.path().join(".agents").join("agents");
```

- [ ] **Step 2: Verify test compiles**

```bash
cargo check -p vol-llm-yaml-agent --tests
```
Expected: no errors

---

### Task 5: Run workspace-wide tests and commit

**Files:** No file changes — verification and commit step.

- [ ] **Step 1: Full workspace check**

```bash
cargo check --workspace
```
Expected: no errors

- [ ] **Step 2: Run affected crate tests**

```bash
cargo test -p vol-llm-wiki -p vol-llm-yaml-agent
```
Expected: all tests pass (note: wiki integration test is `#[ignore]` and will be skipped)

- [ ] **Step 3: Commit code changes**

```bash
git add crates/vol-llm-wiki/src/lib.rs \
    crates/vol-llm-wiki/src/config.rs \
    crates/vol-llm-wiki/src/loader.rs \
    crates/vol-llm-wiki/src/injector.rs \
    crates/vol-llm-wiki/tests/wiki_integration_test.rs \
    crates/vol-llm-yaml-agent/src/lib.rs \
    crates/vol-llm-yaml-agent/src/config.rs \
    crates/vol-llm-yaml-agent/src/discovery.rs

git commit -m "$(cat <<'EOF'
fix: rename .agents/ to .agents/ across vol-llm-wiki and vol-llm-yaml-agent

Unified directory convention: .agents/ (plural) for all agent files.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>
EOF
)"
```

---

### Task 6: Move existing wiki files from `.agents/` to `.agents/`

**Files:**
- Move: `.agents/wikis/*` → `.agents/wikis/`

- [ ] **Step 1: Check current `.agents/` contents**

```bash
ls -la .agents/ 2>/dev/null || echo "No .agents/ directory"
ls -la .agents/wikis/ 2>/dev/null || echo "No .agents/wikis/ directory"
```

- [ ] **Step 2: Move wiki files if `.agents/wikis/` exists**

```bash
if [ -d ".agents/wikis" ]; then
    mkdir -p .agents/wikis
    # Move existing wiki content (preserve subdirectory structure)
    cp -r .agents/wikis/* .agents/wikis/
    echo "Moved wiki files to .agents/wikis/"
    # Verify
    find .agents/wikis -type f | sort
else
    echo "No .agents/wikis/ to move"
fi
```

- [ ] **Step 3: Move any YAML agent files if `.agents/agents/` exists**

```bash
if [ -d ".agents/agents" ]; then
    mkdir -p .agents/agents
    cp -r .agents/agents/* .agents/agents/
    echo "Moved agent YAML files to .agents/agents/"
else
    echo "No .agents/agents/ to move"
fi
```

- [ ] **Step 4: Remove empty `.agents/` directory**

```bash
# Only remove if empty after moves
if [ -d ".agent" ] && [ -z "$(ls -A .agent 2>/dev/null)" ]; then
    rm -rf .agent
    echo "Removed empty .agents/ directory"
else
    echo ".agents/ not empty or doesn't exist, checking subdirs..."
    # Remove empty subdirs
    rmdir .agents/wikis 2>/dev/null || true
    rmdir .agents/agents 2>/dev/null || true
    rmdir .agent 2>/dev/null || true
fi
```

- [ ] **Step 5: Verify `.agents/` structure**

```bash
find .agents -type f | sort
```

Expected output should include:
```
.agents/skills/...
.agents/wikis/INDEX.md
.agents/wikis/LOG.md
.agents/wikis/synthesis.md
.agents/wikis/entities/...
.agents/wikis/concepts/...
.agents/wikis/sources/...
```

- [ ] **Step 6: Commit file moves**

```bash
git add .agents/wikis/
git rm -rf .agents/ 2>/dev/null || true
git status

git commit -m "$(cat <<'EOF'
chore: move existing wiki files from .agents/ to .agents/

Consolidate under unified .agents/ directory.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>
EOF
)"
```

---

### Task 7: Update spec and plan documentation

**Files to update (grep-verified):**
- `docs/superpowers/specs/2026-04-25-coding-agent-skill-session-integration-design.md:62,63,73`
- `docs/superpowers/specs/2026-04-27-wiki-system-design.md:5,15,26,64,98,104`
- `docs/superpowers/specs/2026-04-28-yaml-as-agent-design.md:10,20,21,48`
- `docs/superpowers/plans/2026-04-25-coding-agent-skill-session-integration.md:5,27,30,126,185,186,274,351,352`
- `docs/superpowers/plans/2026-04-27-wiki-system.md:7,66,137,373,545`
- `docs/superpowers/plans/2026-04-28-yaml-as-agent.md:59,231,232,823`
- `docs/superpowers/plans/2026-05-01-md-frontmatter-migration-plan.md:485`

- [ ] **Step 1: Bulk replace in specs**

```bash
sed -i 's|\.agents/|\.agents/|g' \
  docs/superpowers/specs/2026-04-25-coding-agent-skill-session-integration-design.md \
  docs/superpowers/specs/2026-04-27-wiki-system-design.md \
  docs/superpowers/specs/2026-04-28-yaml-as-agent-design.md
```

- [ ] **Step 2: Bulk replace in plans**

```bash
sed -i 's|\.agents/|\.agents/|g' \
  docs/superpowers/plans/2026-04-25-coding-agent-skill-session-integration.md \
  docs/superpowers/plans/2026-04-27-wiki-system.md \
  docs/superpowers/plans/2026-04-28-yaml-as-agent.md \
  docs/superpowers/plans/2026-05-01-md-frontmatter-migration-plan.md
```

- [ ] **Step 3: Verify no remaining `.agents/` references (excluding `.agents/`)**

```bash
grep -rn '\.agents/' docs/superpowers/ | grep -v '\.agents/' | grep -v '\.agents-directory-design'
```
Expected: no output (or only the design spec itself mentioning the old name in context)

- [ ] **Step 4: Commit documentation updates**

```bash
git add docs/superpowers/specs/2026-04-25-coding-agent-skill-session-integration-design.md \
    docs/superpowers/specs/2026-04-27-wiki-system-design.md \
    docs/superpowers/specs/2026-04-28-yaml-as-agent-design.md \
    docs/superpowers/plans/2026-04-25-coding-agent-skill-session-integration.md \
    docs/superpowers/plans/2026-04-27-wiki-system.md \
    docs/superpowers/plans/2026-04-28-yaml-as-agent.md \
    docs/superpowers/plans/2026-05-01-md-frontmatter-migration-plan.md

git commit -m "$(cat <<'EOF'
docs: update spec and plan references from .agents/ to .agents/

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>
EOF
)"
```

---

### Task 8: Update wiki pages to reflect new paths

**Files:**
- Modify: `.agents/wikis/INDEX.md` (update source link paths if needed)
- Modify: `.agents/wikis/sources/wiki-system-design.md` (update spec references)
- Modify: `.agents/wikis/sources/yaml-agent.md` (update spec references)

- [ ] **Step 1: Check wiki pages for `.agents/` references**

```bash
grep -rn '\.agents/' .agents/wikis/ | grep -v '\.agents/'
```

- [ ] **Step 2: Fix any remaining references**

If the grep from Step 1 returns results, use `Edit` to replace each `.agents/` with `.agents/` in the wiki pages.

- [ ] **Step 3: Commit wiki updates**

```bash
git add .agents/wikis/
git commit -m "$(cat <<'EOF'
docs: update wiki pages to use .agents/ convention

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>
EOF
)"
```
