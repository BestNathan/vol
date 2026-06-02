---
name: wiki-query
description: Use when answering questions about the project by searching the wiki at docs/wiki — finding relevant pages, synthesizing answers with citations, and filing good answers back into the wiki
---

# Wiki Query

## Overview

Answer questions by reading the wiki at `docs/wiki/`. The wiki is the persistent knowledge base — use it as the primary source of truth.

## Workflow

### 1. Find relevant pages

Read `docs/wiki/index.md` first. It catalogs all entities, concepts, sources, and analyses with summaries. Use it to identify which pages are relevant to the question.

For more specific searches, use `grep` or `find` in `docs/wiki/`:
```bash
grep -r "keyword" docs/wiki/
find docs/wiki/ -name "*.md" | grep concepts/  # or entities/, sources/
```

### 2. Read and synthesize

Read the relevant wiki pages. Synthesize an answer from the compiled knowledge. **Always cite sources** using wikilink format `[[page-slug]]`.

### 3. Answer formats

Answers can take different forms depending on the question:

- **Explanation**: text with citations
- **Comparison**: markdown table
- **Architecture**: ASCII diagram + explanation
- **Code reference**: point to the actual source file

### 4. File good answers back into the wiki

If the answer reveals a new insight, comparison, or connection that doesn't exist in the wiki:

- Create `docs/wiki/analyses/{slug}.md` with frontmatter:
  ```yaml
  type: analysis
  category: comparison|insight|decision
  tags: [tags]
  created: 2026-05-14
  source_count: N
  ```
- Update `index.md` with the new analysis entry
- Update `log.md`:
  ```markdown
  ## [2026-05-14] query | Question topic
  - Created analyses: [[analysis-slug]]
  - Question: what was asked
  - Answer: brief summary of findings
  - Cross-references added: N
  ```

## When NOT to use this skill

- For questions about files not in the wiki — read the source code directly
- For questions about git history — use `git log` / `git blame`
- For real-time information (deploy status, logs) — check running systems

## Quick reference

| Need | Action |
|------|--------|
| Broad topic overview | Read index.md → read top 3 related pages |
| Specific fact | Grep in docs/wiki/ → read the matching page |
| "How does X work?" | Read the concept page → follow related links |
| "What changed in Y?" | Read the entity page → check timeline |
| Comparison | Read both pages → synthesize → file as analysis |

## Common mistakes

- Answering from memory instead of reading the wiki — the wiki is the source of truth
- Not filing good answers back — insights compound only if persisted
- Citing sources that don't exist — verify wikilinks point to real pages
