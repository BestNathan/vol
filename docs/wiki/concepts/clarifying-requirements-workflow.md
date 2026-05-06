---
type: concept
category: workflow
tags: [requirements, clarification, workflow, skills]
created: 2026-05-06
updated: 2026-05-06
source_count: 1
---

# Clarifying Requirements Workflow

**Category:** Requirements gathering workflow
**Related:** [[skill-system]], [[subagent-review-pattern]], [[human-in-the-loop]]

## Definition

A structured dialogue process for turning vague user requests into precise, actionable requirements documents. Ensures understanding of WHAT to build before exploring HOW to build it.

## Key Points
- **HARD-GATE**: No code reading, no implementation, no brainstorming until request is restated AND at least one clarifying question is answered [[clarifying-requirements-subagent-review]]
- **Sequential checklist**: Restate → Ask questions → Explore context → Follow-up → Edge cases → Success criteria → Write doc → Self-review → Subagent review → User review → Brainstorming [[clarifying-requirements-subagent-review]]
- **One question at a time**: Never batch multiple questions — prevents overwhelming the user [[clarifying-requirements-subagent-review]]
- **WHAT not HOW**: Requirements describe capabilities, not implementation choices [[clarifying-requirements-subagent-review]]
- **Requirements document structure**: Background, Goals, Non-Goals, Scope, Constraints, Success Criteria, Edge Cases, Open Questions [[clarifying-requirements-subagent-review]]
- **Subagent review**: Independent agent checks completeness, consistency, clarity, scope, YAGNI, and measurability before user review [[clarifying-requirements-subagent-review]]

## How It Works

The workflow runs between the user's initial request and any design/brainstorming work:

1. Restate the request in your own words, confirm understanding
2. Ask clarifying questions (one at a time, multiple choice preferred)
3. Explore the current codebase context (read-only, after initial clarification)
4. Ask follow-up questions based on what was found
5. Identify edge cases and define explicit non-goals
6. Write requirements document to `docs/superpowers/requirement/YYYY-MM-DD-<topic>-requirement.md`
7. Self-review: check for placeholders, contradictions, scope, ambiguity
8. Subagent review: dispatch independent reviewer with prompt template
9. User review: present document for confirmation
10. Transition to brainstorming skill

## Pipeline Position

```
User Request → clarifying-requirements → brainstorming → writing-plans → executing-plans
   (WHAT)           (clarify)            (HOW)         (steps)       (build)
```

## Related Concepts
- [[skill-system]]: How skills are loaded and used by agents
- [[subagent-review-pattern]]: The independent review step
- [[human-in-the-loop]]: User review as the final approval gate
- [[react-pattern]]: The broader agent execution loop where this skill operates
- [[agent-builder-pattern]]: Where skills are configured on agents
