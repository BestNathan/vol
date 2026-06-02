#!/bin/bash
# wiki-context.sh — SessionStart hook
# Injects wiki index.md + available wiki skills into the session context.

PROJECT_DIR="${CLAUDE_PROJECT_DIR:-.}"
WIKI_DIR="$PROJECT_DIR/docs/wiki"

if [ ! -d "$WIKI_DIR" ]; then
  exit 0
fi

count_md() {
  find "$1" -name "*.md" 2>/dev/null | wc -l || echo 0
}

# --- Build context text ---
N_CONCEPTS=$(count_md "$WIKI_DIR/concepts")
N_ENTITIES=$(count_md "$WIKI_DIR/entities")
N_SOURCES=$(count_md "$WIKI_DIR/sources")
N_ANALYSES=$(count_md "$WIKI_DIR/analyses")

LAST_ENTRY=""
if [ -f "$WIKI_DIR/log.md" ]; then
  LAST_ENTRY=$(grep "^## \[" "$WIKI_DIR/log.md" | head -1 | sed 's/## //') || true
fi

CONTEXT="Wiki at docs/wiki/ — $N_CONCEPTS concepts, $N_ENTITIES entities, $N_SOURCES sources, $N_ANALYSES analyses."
if [ -n "$LAST_ENTRY" ]; then
  CONTEXT="$CONTEXT Last activity: $LAST_ENTRY."
fi

# --- Inject full index.md ---
if [ -f "$WIKI_DIR/index.md" ]; then
  INDEX_CONTENT=$(cat "$WIKI_DIR/index.md")
  CONTEXT="${CONTEXT}

=== Wiki Index ===
${INDEX_CONTENT}"
fi

# --- Inject available wiki skills ---
SKILLS_DIR="${CLAUDE_SKILLS_DIR:-$PROJECT_DIR/.claude/skills}"
SKILL_TEXT=""
for skill_dir in "$SKILLS_DIR"/wiki-*/; do
  [ -f "$skill_dir/SKILL.md" ] || continue
  skill_name=$(basename "$skill_dir")
  desc=$(grep "^description:" "$skill_dir/SKILL.md" | sed 's/^description: *//') || true
  SKILL_TEXT="${SKILL_TEXT}
### ${skill_name}
${desc}
"
done

if [ -n "$SKILL_TEXT" ]; then
  CONTEXT="${CONTEXT}

=== Available Wiki Skills ===
${SKILL_TEXT}"
fi

# --- Output as JSON ---
printf '%s' "$CONTEXT" | jq -R -s '{
  "hookSpecificOutput": {
    "hookEventName": "SessionStart",
    "additionalContext": .
  }
}'
