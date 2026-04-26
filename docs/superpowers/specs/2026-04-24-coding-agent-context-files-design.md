# CodingAgent Context Files Design

> Status: approved — 2026-04-24

## Problem

CodingAgent currently has a static system prompt. There's no mechanism to load project-specific context files (agent identity, instructions, CLI reference) into the system prompt at startup.

## Solution

Add `init_context_files()` to CodingAgent that generates context files from built-in templates to the project directory (skipping existing). Wire a `FileContributor` into the system prompt to load `AGENT.md`, `INSTRUCTION.md`, `CLI.md` at startup.

## Context File Convention

Three files in the project root (`working_dir`):

| File | Purpose |
|------|---------|
| `AGENT.md` | Agent identity, behavior rules, code style |
| `INSTRUCTION.md` | Project-specific instructions, constraints |
| `CLI.md` | CLI tools reference, build/test/run commands |

## Design

### 1. `init_context_files()` — File Generation

Method on `CodingAgent`:

```
init_context_files(&self) -> Result<(), Error>
```

- Scans `config.working_dir` for each of the three files
- If a file exists, skip it silently
- If missing, generate from a built-in template
- Template content is minimal placeholders — not loaded into prompt directly

### 2. Context Loading at Run Time

CodingAgent reads the three files at startup and injects their content into the system prompt. Missing files are silently skipped.

The loaded content is appended to the existing `prompt_context` system message.

### 3. Templates

Built-in templates are static strings embedded in the crate:

```rust
const AGENT_MD_TEMPLATE: &str = "# Agent\n\nDefine your role and behavior here.\n";
const INSTRUCTION_MD_TEMPLATE: &str = "# Instructions\n\nAdd project-specific instructions here.\n";
const CLI_MD_TEMPLATE: &str = "# CLI Reference\n\nDocument available CLI tools and commands here.\n";
```

## Architecture

```
CodingAgent::new()
  └── reads AGENT.md, INSTRUCTION.md, CLI.md from working_dir
  └── concatenates content → appends to system prompt

CodingAgent::init_context_files()
  └── generates missing files from templates
```

No changes to ContextBuilder or ContextContributor — this is a CodingAgent-local concern.

## Error Handling

- File not found during load → skip silently
- File exists during init → skip silently
- IO error during init → log warning, skip file
