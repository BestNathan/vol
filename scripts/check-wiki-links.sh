#!/usr/bin/env bash
# Validate wiki links for pre-commit and CI.
#
# Catches the two classes of broken link that have bitten this repo:
#   1. [[wiki-links]] whose slug does not resolve to any page under docs/wiki/
#      (e.g. [[vol-llm-task]] when the page is named vol-llm-task-crate.md)
#   2. Relative markdown links [text](path) that point at a file that does not
#      exist on disk (e.g. linking into docs/superpowers/ from inside the wiki).
#
# Designed for AI self-fix: every broken link is reported with
#   file:line:  [[slug]]  → did you mean [[closest-match]]?
# so an agent can apply the fix without re-running the check to discover it.
#
# Usage:
#   ./scripts/check-wiki-links.sh            # check staged wiki files (pre-commit)
#   ./scripts/check-wiki-links.sh --all      # check every wiki file (CI / manual)
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WIKI_DIR="$REPO_ROOT/docs/wiki"
WIKI_SUBDIRS="concepts entities sources analyses"

# ---- collect the page set (slugs that resolve) ----------------------------
declare -A KNOWN_SLUGS=()
for subdir in $WIKI_SUBDIRS; do
  if [ -d "$WIKI_DIR/$subdir" ]; then
    while IFS= read -r f; do
      slug="$(basename "$f" .md)"
      KNOWN_SLUGS["$slug"]=1
    done < <(cd "$REPO_ROOT" && find "docs/wiki/$subdir" -maxdepth 1 -name '*.md' -type f)
  fi
done

# ---- pick which files to scan ---------------------------------------------
MODE="${1:-staged}"
if [ "$MODE" = "--all" ]; then
  mapfile -t FILES < <(cd "$REPO_ROOT" && find docs/wiki -name '*.md' -type f)
else
  mapfile -t FILES < <(git diff --cached --name-only --diff-filter=ACM \
                         | grep '^docs/wiki/.*\.md$' || true)
fi

[ "${#FILES[@]}" -eq 0 ] && exit 0

# ---- helpers --------------------------------------------------------------
# Two-pass similarity:
#   Pass 1 — exact substring containment (high confidence)
#   Pass 2 — "normalize" comparison: strip dashes/underscores, then compare.
#            Catches the `jsonrpc-websocket` → `json-rpc-websocket` class
#            of typo without needing a full Levenshtein implementation.
#   Pass 3 — length-distance fallback for candidates of similar size.
suggest_slug() {
  local target="$1"
  local norm_target="${target//-/}"
  norm_target="${norm_target//_/}"

  local best="" best_score=0

  for slug in "${!KNOWN_SLUGS[@]}"; do
    local score=0

    # Pass 1: exact substring
    if [[ "$slug" == *"$target"* || "$target" == *"$slug"* ]]; then
      score=90
    else
      # Pass 2: normalized (dash/underscore stripped) exact match
      local norm_slug="${slug//-/}"
      norm_slug="${norm_slug//_/}"
      if [ "$norm_target" = "$norm_slug" ]; then
        score=95   # stronger than substring — almost certainly the answer
      elif [[ "$norm_slug" == *"$norm_target"* || "$norm_target" == *"$norm_slug"* ]]; then
        score=80
      else
        # Pass 3: length distance (only close-length candidates)
        local len_t=${#target} len_s=${#slug}
        local diff=$(( len_t - len_s ))
        [ "$diff" -lt 0 ] && diff=$(( -diff ))
        [ "$diff" -gt 6 ] && continue
        score=$(( 70 - diff * 8 ))
      fi
    fi

    if [ "$score" -gt "$best_score" ]; then
      best="$slug"; best_score=$score
    fi
  done
  [ "$best_score" -ge 50 ] && echo "$best"
}

BROKEN_WIKI=0
BROKEN_MD=0
ERRORS_FILE="$(mktemp)"
trap 'rm -f "$ERRORS_FILE"' EXIT

# ---- Pass 1: [[wiki-links]] -----------------------------------------------
for f in "${FILES[@]}"; do
  [ -f "$REPO_ROOT/$f" ] || continue
  lineno=0
  while IFS= read -r line; do
    lineno=$((lineno + 1))
    # Strip inline code spans (`...`) before scanning — documentation that
    # *describes* the wiki-link syntax (like this script's own log entry)
    # would otherwise trigger false positives. sed 's/`[^`]*`//g' removes
    # each paired span; a lone backtick is treated as non-code.
    cleaned="$(printf '%s' "$line" | sed 's/`[^`]*`//g')"
    while [[ "$cleaned" =~ \[\[([a-zA-Z0-9_.-]+)\]\] ]]; do
      slug="${BASH_REMATCH[1]}"
      cleaned="${cleaned#*"${BASH_REMATCH[0]}"}"
      if [ -z "${KNOWN_SLUGS[$slug]+x}" ]; then
        BROKEN_WIKI=$((BROKEN_WIKI + 1))
        suggestion="$(suggest_slug "$slug")"
        if [ -n "$suggestion" ]; then
          echo "  $f:$lineno: [[${slug}]]  →  did you mean [[${suggestion}]]?" >> "$ERRORS_FILE"
        else
          echo "  $f:$lineno: [[${slug}]]  →  no page with this slug exists" >> "$ERRORS_FILE"
        fi
      fi
    done
  done < "$REPO_ROOT/$f"
done

# ---- Pass 2: relative markdown links --------------------------------------
# Catches [text](../../superpowers/specs/foo.md) whose target is outside
# docs/wiki/ (mkdocs rejects these too). Skips http(s)/mailto/anchors.
for f in "${FILES[@]}"; do
  [ -f "$REPO_ROOT/$f" ] || continue
  file_dir="$(dirname "$REPO_ROOT/$f")"
  lineno=0
  while IFS= read -r line; do
    lineno=$((lineno + 1))
    # strip inline code spans here too
    cleaned="$(printf '%s' "$line" | sed 's/`[^`]*`//g')"
    remaining="$cleaned"
    while [[ "$remaining" =~ \]\(([^\)]+)\) ]]; do
      target="${BASH_REMATCH[1]}"
      remaining="${remaining#*"${BASH_REMATCH[0]}"}"
      # strip anchor
      target="${target%%#*}"
      # skip non-file targets
      [[ "$target" =~ ^(https?://|mailto:|$) ]] && continue
      resolved="$file_dir/$target"
      # normalize .. segments without requiring realpath (which follows symlinks)
      resolved="$(cd "$file_dir" 2>/dev/null && realpath -m "$target" 2>/dev/null || echo "$resolved")"
      if [ ! -f "$resolved" ]; then
        BROKEN_MD=$((BROKEN_MD + 1))
        echo "  $f:$lineno: [..]($target)  →  file not found (resolved: ${resolved#$REPO_ROOT/})" >> "$ERRORS_FILE"
      fi
    done
  done < "$REPO_ROOT/$f"
done

# ---- report ---------------------------------------------------------------
TOTAL=$((BROKEN_WIKI + BROKEN_MD))
if [ "$TOTAL" -gt 0 ]; then
  echo ""
  echo "❌ check-wiki-links: $TOTAL broken link(s) in docs/wiki/"
  echo ""
  if [ "$BROKEN_WIKI" -gt 0 ]; then
    echo "  Broken [[wiki-links]] ($BROKEN_WIKI):"
  fi
  if [ "$BROKEN_MD" -gt 0 ]; then
    echo "  Broken markdown links ($BROKEN_MD):"
  fi
  cat "$ERRORS_FILE"
  echo ""
  echo "Fix each line listed above. For [[wiki-links]], the slug must match"
  echo "a .md filename (without extension) under docs/wiki/{concepts,entities,sources,analyses}/."
  echo "For relative markdown links, the target must exist on disk."
  echo ""
  exit 1
fi

echo "check-wiki-links: ✓ (${#FILES[@]} file(s), 0 broken links)"
exit 0
