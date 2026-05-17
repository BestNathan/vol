---
name: wiki-schema
description: Use when setting up a wiki knowledge base, defining wiki conventions, creating page templates, or when wiki pages lack consistent structure, metadata, or cross-referencing rules. Also use when templates exist but agents ignore them, or when CLAUDE.md/schema is missing or thin.
---

# wiki-schema

## Overview

Defines the authoritative schema for a persistent LLM-maintained wiki. This is the contract that governs how the wiki is structured, what conventions pages must follow, and what the LLM must enforce on every operation.

**Core principle: Templates are contracts the LLM must follow, not suggestions for the user.** If a template exists, the LLM uses it for every page it creates or updates.

## Schema Architecture

The wiki has three layers:

```
raw/          ← Immutable source documents (papers, articles, transcripts)
wiki/         ← LLM-generated and maintained markdown pages
CLAUDE.md     ← Schema conventions (this document lives here or in wiki/)
```

## Required Directory Structure

```
wiki/
├── CLAUDE.md           # Schema conventions (REQUIRED)
├── index.md            # Content catalog with summaries
├── log.md              # Append-only change log
├── entities/           # Concrete things: people, orgs, products, events
├── concepts/           # Abstract ideas: techniques, theories, patterns
├── sources/            # Processed summaries of raw sources
└── analyses/           # Synthesis pages: comparisons, deep-dives, answers to queries
```

**No other top-level directories.** If a domain needs more, nest inside these categories.

## Page Templates

Every page type has a mandatory frontmatter and body structure. The LLM MUST use these templates for every page it creates.

### Entity Page (`entities/{slug}.md`)

```markdown
---
type: entity
category: person|org|product|event
tags: [tag1, tag2]
created: 2026-05-01
updated: 2026-05-01
source_count: 1
---

# Entity Name

**Category:** One-line classification
**Related:** [[concept1]], [[concept2]], [[entity2]]

## Overview
2-3 sentence description of what this entity is.

## Key Facts
- Fact 1 with [[source-reference]]
- Fact 2 with [[source-reference]]

## Timeline
- **YYYY-MM**: Event description
```

### Concept Page (`concepts/{slug}.md`)

```markdown
---
type: concept
category: technique|theory|pattern|framework
tags: [tag1, tag2]
created: 2026-05-01
updated: 2026-05-01
source_count: 1
---

# Concept Name

**Category:** One-line classification
**Related:** [[related-concept]], [[entity1]]

## Definition
Clear, concise definition in 1-2 sentences.

## Key Points
- Point 1 with [[source-reference]]
- Point 2 with [[source-reference]]

## How It Works
Brief explanation of mechanism or process.

## Examples / Applications
- Example 1 with context

## Related Concepts
- [[concept-a]]: How it relates
- [[concept-b]]: How it differs
```

### Source Summary (`sources/{slug}.md`)

```markdown
---
type: source
source_type: paper|article|transcript|report|book
date: 2026-05-01
ingested: 2026-05-01
tags: [tag1, tag2]
---

# Source Title

**Authors/Creators:** Name(s)
**Date:** YYYY-MM or YYYY
**Link:** URL or citation

## TL;DR
One sentence summary.

## Key Takeaways
- Takeaway 1
- Takeaway 2

## Detailed Summary
2-4 paragraphs of substantive summary.

## Entities Mentioned
- [[entity1]]: Role in this source

## Concepts Covered
- [[concept1]]: What this source says about it

## Notes
Additional observations, contradictions, or connections to other wiki pages.
```

### Analysis Page (`analyses/{slug}.md`)

```markdown
---
type: analysis
created: 2026-05-01
tags: [tag1, tag2]
---

# Analysis Title

**Question/Topic:** What this analysis addresses

## Summary
Key finding or conclusion in 1-2 sentences.

## Analysis
Detailed reasoning with [[wiki-page]] citations throughout.

## Sources
Pages consulted during this analysis:
- [[page1]]
- [[page2]]
```

## Naming Conventions

| Rule | Format | Example |
|------|--------|---------|
| Slugs | lowercase, hyphens | `self-supervised-learning.md` |
| Disambiguation | Add year or context suffix | `transformers-2017.md` vs `transformers-toys.md` |
| Frontmatter tags | Array of lowercase hyphenated strings | `tags: [deep-learning, attention]` |
| Wiki links | `[[slug-without-ext]]` | `[[self-supervised-learning]]` |
| Dates | ISO 8601 | `2026-05-01` |

## Status State Machine

Pages in `index.md` use these exact status values — no others:

| Status | Meaning |
|--------|---------|
| `stub` | Page exists with minimal content, needs expansion |
| `active` | Being actively developed, new sources being added |
| `stable` | Well-developed, only minor updates expected |
| `stale` | No updates in 60+ days, may contain outdated claims |

## Cross-Reference Conventions

**Bidirectional linking is mandatory.** When page A links to page B, page B's "Related" section must include A.

**Link format:** Use `[[slug]]` syntax (wiki-style, without `.md` extension). This makes links parseable and enables tool-based graph analysis.

**On ingest, the LLM must:**
1. Identify entities and concepts mentioned in the new source
2. Create or update those pages
3. Add `[[new-source]]` links in relevant existing pages
4. Add back-links from new pages to existing ones

## index.md Format

```markdown
# Wiki Index

Last updated: YYYY-MM-DD

## Entities
| Page | Summary | Status | Updated |
|------|---------|--------|---------|
| [[name]] | One-line summary | active | YYYY-MM-DD |

## Concepts
(same table format)

## Sources
(same table format)

## Analyses
(same table format)
```

## log.md Format

```markdown
# Change Log

## [YYYY-MM-DD] ingest | Source Title
- Created: [[source-slug]]
- Updated: [[concept1]], [[entity2]]
- Cross-references added: 3

## [YYYY-MM-DD] query | Topic Question
- Created: [[analysis-slug]]
- Pages consulted: [[page1]], [[page2]]

## [YYYY-MM-DD] lint | Health Check
- Found: 1 contradiction, 2 stale claims, 1 orphan
- Fixed: index updated, 3 cross-refs added
```

Prefix format is parseable with `grep "^## \[" log.md | tail -5`.

## CLAUDE.md Requirements

When setting up a wiki, the CLAUDE.md MUST include:

1. **This schema** (or a link to it) — directory structure, templates, conventions
2. **Workflow instructions** — what to do on ingest, query, and lint commands
3. **Quality bar** — "every page must follow its template" not "use templates as guidance"
4. **Tag guidance** — either a controlled vocabulary or a rule for consistent tag creation

## Common Mistakes

| Mistake | Fix |
|---------|-----|
| Creating templates but not using them | Templates are contracts — use them for every page |
| Ambiguous status values ("Started" vs "In Progress") | Use only: stub, active, stable, stale |
| No cross-reference convention | Use `[[slug]]` syntax, enforce bidirectional linking |
| CLAUDE.md omits log.md conventions | Include both index.md and log.md format specs |
| No disambiguation rule for similar names | Add year or context suffix |

## Real-World Impact

Without this schema, agents naturally:
- Create templates they ignore (baseline: 3 topic READMEs with different structure than the template)
- Produce thin CLAUDE.md files with no data model or metadata schema
- Leave cross-references inconsistent, turning the wiki into isolated pages
- Use ambiguous status values that make the index useless for tracking progress
