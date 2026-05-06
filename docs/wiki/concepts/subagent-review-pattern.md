---
type: concept
category: pattern
tags: [subagent-review, document-review, quality-gate]
created: 2026-05-06
updated: 2026-05-06
source_count: 2
---

# Subagent Review Pattern

**Category:** Quality gate pattern
**Related:** [[skill-system]], [[human-in-the-loop]], [[clarifying-requirements-workflow]]

## Definition

A two-stage document review pattern where an independent subagent reviews a document (spec, requirements) after self-review and before the user review gate. This provides fresh-eye verification without consuming user time on obvious issues.

## Key Points
- **Two-stage review**: Self-review (inline, quick checks) → Subagent review (independent, thorough) → User review (final approval) [[clarifying-requirements-subagent-review]]
- **Prompt templates**: Reusable prompt templates (`*-reviewer-prompt.md`) standardize what the subagent checks [[clarifying-requirements-subagent-review]]
- **Calibration**: Subagents are instructed to only flag issues that would cause real problems — minor stylistic preferences don't block approval [[clarifying-requirements-subagent-review]]
- **Fix inline**: If the subagent finds issues, they're fixed inline without requiring a re-review [[clarifying-requirements-subagent-review]]
- **Independent perspective**: Subagent brings fresh eyes, catching contradictions and gaps the author missed [[clarifying-requirements-subagent-review]]

## How It Works

The pattern consists of three steps:

1. **Self-review**: Author scans for placeholders, internal consistency, scope, and ambiguity
2. **Subagent review**: Dispatch a general-purpose agent with a prompt template that specifies what to check
3. **User review**: Only after both automated checks pass, present the document to the user

```
Write Document → Self-Review (inline) → Subagent Review? → User Review? → Proceed
                                         │                    │
                                         └─ issues → Fix ─────┘
```

## Implementations

| Skill | Reviewer Template | Checks |
|-------|-------------------|--------|
| brainstorming | `spec-document-reviewer-prompt.md` | Completeness, Consistency, Clarity, Scope, YAGNI |
| clarifying-requirements | `requirements-document-reviewer-prompt.md` | Completeness, Consistency, Clarity, WHAT-not-HOW, Scope, YAGNI, Measurability |

## Related Concepts
- [[skill-system]]: Where the pattern is implemented
- [[human-in-the-loop]]: User review is the final gate after automated reviews
- [[clarifying-requirements-workflow]]: Uses subagent review for requirements documents
- [[built-in-plugins]]: Similar concept — plugins as reusable quality checks in agent execution
