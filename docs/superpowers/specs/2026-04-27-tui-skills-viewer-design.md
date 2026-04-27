# TUI Skills Viewer Design

## Overview

Add a dedicated skills viewer to the TUI so users can see what skills are loaded at startup. Currently, skills are injected into the system prompt where only the LLM can see them — users have no visibility into which skills are active.

## Architecture

### Status Bar Enhancement

Add a `skills_count` field to `AppState`. The status bar line gets a new segment:

```
Session: abc123 | Runs: 2 | Iter: 0 | Tools: 0 | Skills: 5 | 14:32:01
```

If skills_count is 0, the segment is hidden entirely.

### New Skills Tab

Add `ActiveTab::Skills` as the 4th tab (between Workspace and Logs):

```
 Conversation | Workspace | Skills | Logs
```

The Skills panel renders a scrollable table showing:
- **Name** — skill name
- **Version** — version string
- **Scope** — User/Repo/Custom (color-coded: User=green, Repo=blue, Custom=yellow)
- **Description** — truncated to fit available width

Empty state shows a "No skills discovered" message.

### Data Flow

1. `main.rs` creates `SkillLoader::new(Some(working_dir))` at startup
2. Calls `discover_all()` immediately
3. Extracts `Vec<SkillMetadata>` and stores in `AppState.skills`
4. Skills are static at TUI startup — no re-discovery during agent runs

## Files Changed

| File | Change |
|------|--------|
| `crates/vol-llm-tui/src/app.rs` | Add `SkillDisplayEntry` struct, `skills` field on `AppState`, add `ActiveTab::Skills` variant |
| `crates/vol-llm-tui/src/main.rs` | Create `SkillLoader` at startup, discover skills, populate `AppState.skills` |
| `crates/vol-llm-tui/src/ui/mod.rs` | Add `ActiveTab::Skills` to tab bar, add render branch for skills panel |
| `crates/vol-llm-tui/src/ui/status_bar.rs` | Render skills count segment |
| `crates/vol-llm-tui/src/ui/skills_panel.rs` | **New file** — render skills table |
| `crates/vol-llm-tui/Cargo.toml` | Add `vol-llm-skill` dependency |

## Key Decisions

- **Discover at startup, not on demand**: Skills should be visible immediately without triggering an agent run. The slight startup delay from disk I/O is acceptable.
- **No re-discovery during runs**: Skills are considered static for the TUI session. Adding/removing skills requires a restart.
- **Table layout, not card layout**: Skills are simple metadata records — a table fits more content and matches the existing TUI aesthetic.
- **No interactive features beyond scrolling**: The `skill` tool in the agent already handles loading full skill content. The TUI viewer is read-only visibility.

## Error Handling

- If `discover_all()` fails, log a warning and show an empty skills list with status bar count 0
- Non-existent skill directories are silently skipped by `SkillLoader` (existing behavior)
