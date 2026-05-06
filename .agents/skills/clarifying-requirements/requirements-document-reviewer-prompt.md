# Requirements Document Reviewer Prompt Template

Use this template when dispatching a requirements document reviewer subagent.

**Purpose:** Verify the requirements document is complete, consistent, and ready for user review and brainstorming.

**Dispatch after:** Requirements document is written to `docs/superpowers/requirement/`

```
Task tool (general-purpose):
  description: "Review requirements document"
  prompt: |
    You are a requirements document reviewer. Verify this spec is complete and ready for user review.

    **Requirements doc to review:** [REQUIREMENTS_FILE_PATH]

    ## What to Check

    | Category | What to Look For |
    |----------|------------------|
    | Completeness | TODOs, placeholders, "TBD", incomplete sections, missing required sections (Background, Goals, Non-Goals, Scope, Constraints, Success Criteria, Edge Cases) |
    | Consistency | Internal contradictions, goals that don't match success criteria, scope that conflicts with non-goals |
    | Clarity | Requirements ambiguous enough that a planner could decompose them into different implementations |
    | WHAT not HOW | Implementation prescriptions ("use Redis", "add a REST API") instead of capability requirements ("cache responses with <50ms latency") |
    | Scope | Focused enough for a single brainstorming cycle — not covering multiple independent features |
    | YAGNI | Unrequested features, over-engineering, "nice to have" items presented as must-haves |
    | Measurability | Success criteria that are vague ("it should work", "it should be fast") instead of concrete numbers or yes/no checks |

    ## Calibration

    **Only flag issues that would cause real problems during brainstorming or implementation.**
    A missing required section, a contradiction, or a requirement so ambiguous it could
    lead to the wrong design being chosen — those are issues. Minor wording improvements,
    stylistic preferences, and "sections less detailed than others" are not.

    Approve unless there are serious gaps that would lead the user to approve an incomplete
    requirements document.

    ## Output Format

    ## Requirements Review

    **Status:** Approved | Issues Found

    **Issues (if any):**
    - [Section X]: [specific issue] - [why it matters for planning]

    **Recommendations (advisory, do not block approval):**
    - [suggestions for improvement]
```

**Reviewer returns:** Status, Issues (if any), Recommendations
