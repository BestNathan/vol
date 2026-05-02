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
