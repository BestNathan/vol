---
type: source
source_type: design
date: 2026-05-06
ingested: 2026-05-06
tags: [skills, clarifying-requirements, subagent-review]
---

# Clarifying Requirements Subagent Review

**Authors/Creators:** BestNathan
**Date:** 2026-05-06
**Link:** `.claude/skills/clarifying-requirements/requirements-document-reviewer-prompt.md`

## TL;DR
Added a subagent review mechanism to the clarifying-requirements skill, mirroring the spec document reviewer pattern from the brainstorming skill. An independent agent reviews the requirements document before the user review gate.

## Key Takeaways
- Subagent review step inserted between self-review and user review in the clarifying-requirements workflow
- `requirements-document-reviewer-prompt.md` provides a reusable prompt template for dispatching the reviewer
- Review checks: Completeness, Consistency, Clarity, WHAT-not-HOW, Scope, YAGNI, Measurability
- Reviewer uses calibration: only flags issues that would cause real problems during brainstorming/implementation
- Pattern mirrors brainstorming's `spec-document-reviewer-prompt.md` for consistency across skills

## Detailed Summary

The clarifying-requirements skill previously only had an inline self-review step after writing requirements. The brainstorming skill had a more robust pattern with a dedicated subagent reviewer (`spec-document-reviewer-prompt.md`). This change brings the same independent review pattern to requirements documents.

The reviewer subagent checks:
- **Completeness**: All required sections present, no TODOs/TBDs
- **Consistency**: No contradictions between goals, scope, success criteria
- **Clarity**: Requirements specific enough for a planner to decompose into steps
- **WHAT not HOW**: No implementation prescriptions
- **Scope**: Focused enough for a single brainstorming cycle
- **YAGNI**: No over-engineering or unrequested features
- **Measurability**: Concrete success criteria, not "it should work"

If issues are found, they're fixed inline and self-review re-runs. If approved, the document proceeds to user review.

## Entities Mentioned
- [[vol-llm-agent-crate]]: ReActAgent where skills are configured
- [[vol-llm-agents-crate]]: High-level agents that use clarifying-requirements

## Concepts Covered
- [[skill-system]]: Skills as native ReActAgent capability
- [[subagent-review-pattern]]: Independent agent review of documents before user gate
- [[clarifying-requirements-workflow]]: Structured dialogue for turning vague requests into requirements
- [[human-in-the-loop]]: User review gate as the next step after subagent approval

## Notes
- The change aligns both brainstorming and clarifying-requirements skills on the same review pattern: self-review → subagent review → user review
- Both skills now follow a two-stage review process before user involvement
