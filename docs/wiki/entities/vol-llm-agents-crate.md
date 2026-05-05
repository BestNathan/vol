---
type: entity
category: product
tags: [crate, agent, rust, high-level]
created: 2026-05-04
updated: 2026-05-04
source_count: 1
---

# vol-llm-agents Crate

**Category:** Rust crate — High-level agent implementations
**Related:** [[vol-llm-agent-crate]], [[react-pattern]]

## Overview

Contains concrete agent implementations built on top of `vol-llm-agent`: advice agent, coding agent, PPT agent, QA agent, and wiki agent.

## Key Facts
- Provides domain-specific agent implementations [[react-agent-docs]]
- Each agent type lives in its own subdirectory: `advice/`, `coding/`, `ppt/`, `qa/`, `wiki/` [[react-agent-docs]]
- Includes integration and E2E tests for each agent type
- Tests include: coding agent with Deribit WebSocket, agent-Loki integration, PPT agent integration, skill session integration

## Timeline
- **2026-04**: Agent implementations added
- **2026-04**: Integration tests written for all agent types
