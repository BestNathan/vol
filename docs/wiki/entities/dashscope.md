---
type: entity
category: product
tags: [api, llm, dashscope, claude]
created: 2026-05-04
updated: 2026-05-04
source_count: 1
---

# DashScope

**Category:** API endpoint for Claude model access
**Related:** [[vol-llm-provider-crate]], [[react-agent-docs]]

## Overview

DashScope is the API endpoint used to access Claude models (via Anthropic-compatible protocol) for the ReAct Agent.

## Key Facts
- Endpoint: `https://coding.dashscope.aliyuncs.com/apps/anthropic` [[react-agent-docs]]
- Model: `claude-sonnet-4-6` [[react-agent-docs]]
- Uses Coding Plan which only works for Coding Agents — returns 405 for non-coding requests [[react-agent-docs]]
- Anthropic-compatible protocol: same request/response format as native Anthropic API

## Timeline
- **2026-04**: Configured as LLM provider for testing
