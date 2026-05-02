---
name: superpowers-workflow-wiki-sync
description: Design a SessionStart hook that instructs Claude to auto-run wiki-ingest, wiki-lint, and Feishu push after completing a superpowers workflow
type: spec
---

# Superpowers Workflow → Wiki Sync Hook

## Problem

After completing a superpowers workflow (brainstorming → writing-plans → executing-plans → finishing-a-development-branch), the generated design specs and plans exist as markdown files in `docs/superpowers/` but are not integrated into the project wiki knowledge base.

## Solution

A `SessionStart` command hook injects a system-level instruction into every session. The LLM carries this instruction and autonomously runs the wiki pipeline when it detects workflow completion.

## Architecture

```
SessionStart (startup)
    │
    ▼
Shell script outputs systemMessage:
"当检测到 superpowers 工作流完成时，自动执行 wiki 整理"
    │
    ▼
LLM 在会话中带着该指令工作
    │
    ▼
检测到四阶段完成 (brainstorming → writing-plans → executing-plans → finishing)
    │
    ▼
LLM 自动执行:
  1. wiki-ingest (整合 spec/plan → docs/wiki/)
  2. wiki-lint (健康检查 + 自动修复)
  3. lark-cli 推送变更页面到 Feishu wiki (spaceID: 7630485291026910436)
```

## Hook Configuration

Add to `.claude/settings.json` under the `hooks` key:

```json
{
  "hooks": {
    "SessionStart": [
      {
        "matcher": "startup",
        "hooks": [
          {
            "type": "command",
            "command": "\"$CLAUDE_PROJECT_DIR/.claude/hooks/superpowers-wiki-reminder.sh\""
          }
        ]
      }
    ]
  }
}
```

## Hook Script

**File:** `.claude/hooks/superpowers-wiki-reminder.sh`

Reads JSON from stdin (hook payload), writes JSON to stdout with `systemMessage`:

```bash
#!/bin/bash
cat <<'EOF'
{
  "continue": true,
  "systemMessage": "【Wiki Sync Reminder】\nWhen a full superpowers workflow completes in this session (brainstorming → writing-plans → executing-plans → finishing-a-development-branch), automatically:\n1. Run wiki-ingest on new spec/plan files in docs/superpowers/specs/ and docs/superpowers/plans/ to integrate into docs/wiki/\n2. Run wiki-lint on docs/wiki/ to health-check and auto-fix (orphans, broken links, index staleness)\n3. Push only changed wiki pages to Feishu wiki space 7630485291026910436 via lark-cli\nCheck the transcript for skill invocation patterns to detect workflow completion."
}
EOF
exit 0
```

## Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| SessionStart only | Single hook, single source of truth. LLM carries instruction throughout session |
| systemMessage output | Injected into system prompt, persists across context compaction better than additionalContext |
| LLM-driven execution | Not a shell script doing wiki operations — LLM understands content, can properly do knowledge integration |
| Transcript-based detection | LLM reads its own transcript to detect which skills were invoked |

## Why Not Stop Hook?

Stop hook fires after LLM response is complete. The response cycle is over — additionalContext does not trigger a new LLM turn. An agent-type Stop hook could work but adds complexity and cost (spawning a subagent). SessionStart is simpler: the instruction is already in context when the LLM works.

## Trade-offs

| Concern | Mitigation |
|---------|-----------|
| LLM may forget in long sessions | systemMessage is part of system prompt, more resilient to compaction |
| Fires in every session | No-op if no superpowers workflow runs — wiki-ingest/lint only trigger when relevant |
| No explicit "completion gate" | LLM autonomously decides when workflow is done based on skill invocation patterns |

## Files Changed

| File | Action |
|------|--------|
| `.claude/settings.json` | Add SessionStart hook entry |
| `.claude/hooks/superpowers-wiki-reminder.sh` | New file — hook script |
