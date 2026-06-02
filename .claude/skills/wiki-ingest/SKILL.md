---
name: wiki-ingest
description: Use when adding a new source document to the wiki — ingesting code changes, design docs, reports, or any source that should be compiled into the persistent wiki at docs/wiki
---

# Wiki Ingest

## Overview

Transform a raw source document into structured wiki content. The LLM reads the source, extracts key information, and integrates it into the existing wiki — creating/updating entity, concept, source, and analysis pages.

## Workflow

### 1. Read the source

Read the source file or document. Understand what it describes — code, design, incident, decision, etc.

### 2. Create source page

Create `docs/wiki/sources/{slug}.md` with:

```markdown
---
type: source
source_type: report|code|design|incident  # pick appropriate type
date: 2026-05-14                          # source date
ingested: 2026-05-14                      # today
tags: [relevant, tags]
---

# Title

**Authors/Creators:** team or person
**Date:** 2026-05-14
**Link:** path or URL

## TL;DR
One-paragraph summary.

## Key Takeaways
- Bullet list of the most important facts

## Detailed Summary
Expand on the key points. Include code references, architecture decisions, trade-offs.

## Entities Mentioned
- [[existing-entity]]: how it appears
- [[new-entity]]: brief description

## Concepts Covered
- [[existing-concept]]: relationship
- [[new-concept]]: brief description

## Notes
Edge cases, TODOs, follow-ups.
```

### 3. Create/update entity pages

For each **concrete thing** (crates, services, databases, systems) mentioned:
- **New**: create `docs/wiki/entities/{slug}.md` with overview, key facts, module structure
- **Existing**: update with new capabilities, changes, timeline entry

Entity pages use frontmatter:
```yaml
type: entity
category: product|infrastructure|service
tags: [tags]
created: 2026-05-14
updated: 2026-05-14
source_count: N
```

### 4. Create/update concept pages

For each **pattern, technique, or idea**:
- **New**: create `docs/wiki/concepts/{slug}.md` with definition, key points, how it works, examples, related concepts
- **Existing**: update with new information, usage examples

Concept pages use frontmatter:
```yaml
type: concept
category: pattern|technique|architecture
tags: [tags]
created: 2026-05-14
updated: 2026-05-14
source_count: N
```

### 5. Update index.md

Add new entries to the appropriate section (Entities, Concepts, Sources, Analyses) with a one-line summary.

### 6. Update log.md

Append an entry with consistent format:

```markdown
## [2026-05-14] ingest | Source Title
- Created sources: [[slug]]
- Created concepts: [[concept-slug]]
- Updated concepts: [[concept-slug]] (what changed)
- Updated entities: [[entity-slug]] (what changed)
- Updated index: new entries, updated summaries
- Cross-references added: N
- Changes: brief summary of what the source contains and what changed
```

### 7. Update source_count

Increment `source_count` in frontmatter of every updated entity/concept page.

## Cross-reference rules

- Use `[[wikilink]]` format for all internal references
- Every new concept/entity page must link back to the source that created it
- Every source page must list all entities and concepts it touches
- Every concept/entity page must have a "Related" or "Related Concepts" section
- New concept pages must update existing related pages to add forward references

## Common mistakes

- Forgetting to update index.md — the wiki becomes hard to navigate
- Forgetting to update log.md — no audit trail of wiki evolution
- Creating concept pages for things that already exist — check index.md first
- Orphan pages — every page must have at least one inbound link
- Inconsistent wikilink format — always `[[slug]]`, never `[[slug|display]]`
