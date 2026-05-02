# SessionStart Hook for Superpowers Wiki Sync Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a SessionStart hook that injects a wiki sync instruction into every Claude session, triggering wiki-ingest + wiki-lint + Feishu push when a superpowers workflow completes.

**Architecture:** A single shell script outputs a `systemMessage` via JSON stdout. Claude Code loads this into the system prompt at session start. The LLM autonomously detects workflow completion (four skill invocations in transcript) and runs the wiki pipeline.

**Tech Stack:** Bash, Claude Code hooks (SessionStart event), lark-cli

---

### Task 1: Create the hook script

**Files:**
- Create: `.claude/hooks/superpowers-wiki-reminder.sh`

- [ ] **Step 1: Create the hooks directory and script**

```bash
mkdir -p /root/nq-deribit/.claude/hooks
```

- [ ] **Step 2: Write the hook script**

Create `.claude/hooks/superpowers-wiki-reminder.sh`:

```bash
#!/bin/bash
# SessionStart hook: injects wiki-sync instruction when a superpowers
# workflow completes. Reads hook JSON from stdin, writes JSON to stdout.

cat <<'EOF'
{
  "continue": true,
  "systemMessage": "【Wiki Sync Reminder】When a full superpowers workflow completes in this session (all four skills invoked: brainstorming → writing-plans → executing-plans → finishing-a-development-branch), automatically:\n1. Run wiki-ingest on new spec/plan files in docs/superpowers/specs/ and docs/superpowers/plans/ to integrate into .agents/wikis/wiki/\n2. Run wiki-lint on .agents/wikis/wiki/ to health-check and auto-fix (orphans, broken links, index staleness)\n3. Push only changed wiki pages to Feishu wiki space 7630485291026910436 via lark-cli docs +create\nCheck the transcript (available at $transcript_path from hook input) for skill invocation patterns to detect workflow completion."
}
EOF
exit 0
```

- [ ] **Step 3: Make the script executable**

```bash
chmod +x /root/nq-deribit/.claude/hooks/superpowers-wiki-reminder.sh
```

- [ ] **Step 4: Test the script in isolation**

```bash
echo '{"session_id": "test", "cwd": "/root/nq-deribit"}' | bash /root/nq-deribit/.claude/hooks/superpowers-wiki-reminder.sh
```

Expected output: valid JSON with `"continue": true` and a `systemMessage` field containing the wiki sync instructions.

Expected exit code: `0`

- [ ] **Step 5: Validate JSON output**

```bash
echo '{"session_id": "test", "cwd": "/root/nq-deribit"}' | bash /root/nq-deribit/.claude/hooks/superpowers-wiki-reminder.sh | python3 -c "import sys,json; d=json.load(sys.stdin); assert d['continue']==True; assert 'systemMessage' in d; print('OK:', d['systemMessage'][:60]+'...')"
```

Expected: `OK: 【Wiki Sync Reminder】When a full superpowers workflow com...`

- [ ] **Step 6: Commit**

```bash
cd /root/nq-deribit
git add .claude/hooks/superpowers-wiki-reminder.sh
git commit -m "$(cat <<'EOF'
feat(hooks): add SessionStart wiki-sync reminder script

When a superpowers workflow completes, the script outputs a
systemMessage instructing the LLM to run wiki-ingest, wiki-lint,
and push changed pages to Feishu wiki space.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>
EOF
)"
```

---

### Task 2: Configure the hook in project settings

**Files:**
- Create: `.claude/settings.json`

The project-level `.claude/settings.json` does not exist yet. The existing `.claude/settings.local.json` contains permission allowlists and should not be modified (it's not committed). We create a new `.claude/settings.json` with just the hook configuration.

- [ ] **Step 1: Create `.claude/settings.json` with hook configuration**

Create `.claude/settings.json`:

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

- [ ] **Step 2: Validate JSON syntax**

```bash
python3 -c "import json; json.load(open('/root/nq-deribit/.claude/settings.json')); print('Valid JSON')"
```

Expected: `Valid JSON`

- [ ] **Step 3: Commit**

```bash
cd /root/nq-deribit
git add .claude/settings.json
git commit -m "$(cat <<'EOF'
feat(hooks): configure SessionStart hook for wiki-sync reminder

Add a SessionStart command hook that fires on session startup,
injecting wiki-sync instructions into every Claude session.
When a superpowers workflow completes, the LLM will automatically
run wiki-ingest, wiki-lint, and push to Feishu.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>
EOF
)"
```

---

### Task 3: Verify end-to-end integration

**Files:**
- No new files
- Verify: hook script + settings.json work together

- [ ] **Step 1: Verify hook is discoverable by Claude Code**

```bash
cd /root/nq-deribit
python3 -c "
import json
settings = json.load(open('.claude/settings.json'))
hooks = settings.get('hooks', {})
ss = hooks.get('SessionStart', [])
assert len(ss) == 1, 'Expected 1 SessionStart hook'
assert ss[0]['matcher'] == 'startup', 'Expected startup matcher'
assert len(ss[0]['hooks']) == 1, 'Expected 1 hook definition'
assert ss[0]['hooks'][0]['type'] == 'command', 'Expected command type'
print('Hook configuration valid')
"
```

Expected: `Hook configuration valid`

- [ ] **Step 2: Verify script path resolves correctly**

```bash
CLAUDE_PROJECT_DIR=/root/nq-deribit bash -c 'eval "$(cat /root/nq-deribit/.claude/settings.json | python3 -c "import sys,json; print(json.load(sys.stdin)[\"hooks\"][\"SessionStart\"][0][\"hooks\"][0][\"command\"])")"' < /dev/null | python3 -c "import sys,json; json.load(sys.stdin); print('Script resolves and produces valid JSON')"
```

Expected: `Script resolves and produces valid JSON`

- [ ] **Step 3: Commit any remaining changes**

No new files expected from verification.

---

## Self-Review

**1. Spec coverage:**

| Spec Requirement | Task |
|------------------|------|
| SessionStart hook injects systemMessage | Task 1 (script), Task 2 (config) |
| Detects four-stage workflow completion | Instruction embedded in systemMessage tells LLM to check transcript |
| wiki-ingest on new spec/plan files | Instruction embedded in systemMessage |
| wiki-lint health check | Instruction embedded in systemMessage |
| Push only changed pages to Feishu | Instruction embedded in systemMessage |
| Wiki path: .agents/wikis/wiki/ | Correct in both script and systemMessage |
| Feishu spaceID: 7630485291026910436 | Correct in systemMessage |

**2. Placeholder scan:** No TBD, TODO, or vague instructions found. All code and commands are complete.

**3. Type consistency:** Not applicable — this is a shell script + JSON config, no types.
