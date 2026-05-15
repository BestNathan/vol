---
name: wiki-lint
description: Use when health-checking the wiki at docs/wiki — finding contradictions, stale claims, orphan pages, missing cross-references, and data gaps to keep the wiki accurate and navigable
---

# Wiki Lint

## Overview

Health-check the wiki to find and fix quality issues. Like a linter for code, but for wiki content.

## Checks

Run these checks in order:

### 1. Orphan pages

```bash
# Find all wikilinks referenced anywhere
grep -roh '\[\[.*\]\]' docs/wiki/ | sort -u > /tmp/references.txt

# Find all actual page files — extract bare slug from path
find docs/wiki/ -name "*.md" -not -name "index.md" -not -name "log.md" | \
  sed 's|docs/wiki/||; s/.md$//; s|^.*/||' | \
  while read -r page; do
    grep -q "\[\[$page\]\]" /tmp/references.txt || echo "ORPHAN: $page"
  done
```

**Fix**: Either add cross-references from related pages, or delete the orphan if no longer relevant.

### 2. Broken wikilinks

```bash
# Extract all [[wikilinks]] from wiki pages
grep -roh '\[\[.*\]\]' docs/wiki/ | sort -u | \
  sed 's/\[\[//; s/\]\]//' > /tmp/all-links.txt

# Check each link has a corresponding page
for link in $(cat /tmp/all-links.txt); do
  if ! find docs/wiki/ -name "${link}.md" -print -quit | grep -q .; then
    echo "BROKEN: $link"
  fi
done
```

**Fix**: Either create the missing page or remove the broken reference.

### 3. Stale claims

Look for pages with `updated:` dates significantly older than the current date. Check if newer sources supersede claims on those pages.

```bash
# Show pages not updated in the last 30 days
grep -r "updated:" docs/wiki/concepts/ docs/wiki/entities/
```

**Fix**: Read the stale page, check newer sources for contradictions, update or mark as `status: stale`.

### 4. Contradictions

Read pages that reference the same concept from different sources. Look for conflicting information about:
- Module structure
- API signatures
- Feature status (active/stable/deleted)
- Architecture decisions

**Fix**: Resolve the contradiction, update both pages to be consistent.

### 5. Missing cross-references

Find pages that mention a concept but don't link to its page:

```bash
# For each concept page, check if source pages that mention it have a link
for concept in docs/wiki/concepts/*.md; do
  name=$(basename "$concept" .md)
  # Search for the concept title in source files without a [[wikilink]]
  grep -rl "$(head -20 "$concept" | grep "^# " | sed 's/# //')" docs/wiki/sources/ | \
    while read -r f; do
      if ! grep -q "\[\[$name\]\]" "$f"; then
        echo "MISSING LINK: $f mentions $name but has no [[wikilink]]"
      fi
    done
done
```

**Fix**: Add the missing `[[wikilink]]` to the page.

### 6. Index/log sync

- Check `index.md` has entries for all pages in concepts/, entities/, sources/, analyses/
- Check `log.md` has recent entries — if the last entry is old, the wiki may be abandoned

**Fix**: Add missing entries to index.md.

## Report format

After running checks, summarize findings:

```
## Wiki Health Report [date]

| Check | Status | Issues |
|-------|--------|--------|
| Orphan pages | OK / N found | |
| Broken links | OK / N found | |
| Stale claims | N pages > 30 days | |
| Contradictions | N found | |
| Missing cross-refs | N found | |
| Index sync | OK / N missing | |
```

Then fix any issues found.

## When to run

- After a major ingest session
- Before closing out a project milestone
- Periodically (weekly/biweekly) on active wikis
