# Tasks: Agent Alert Advice

## Implementation Plan

### Phase 1: Core Infrastructure

- [ ] **1.1** Create `vol-llm-bridge` crate
  - [ ] Cargo.toml with dependencies
  - [ ] src/lib.rs exports
  
- [ ] **1.2** Implement `FrequencyLimiter`
  - [ ] src/limiter.rs
  - [ ] Unit tests for cooldown logic
  - [ ] Unit tests for hourly limit

- [ ] **1.3** Add `AgentAdviceConfig` to vol-config
  - [ ] src/config.rs new struct
  - [ ] Default values
  - [ ] TOML example in docs

### Phase 2: Agent Advice Service

- [ ] **2.1** Implement `AgentAdviceService` skeleton
  - [ ] src/service.rs struct definition
  - [ ] Constructor with config
  - [ ] run() method with broadcast subscription

- [ ] **2.2** Implement history fetching
  - [ ] TdengineClient integration
  - [ ] Query alert_history for 1h/24h data
  - [ ] HistoryData struct

- [ ] **2.3** Implement advice generation
  - [ ] src/prompt.rs with system prompt
  - [ ] ReActAgent integration
  - [ ] parse_advice() function

- [ ] **2.4** Implement Feishu notification
  - [ ] Reuse vol-notification FeishuNotification
  - [ ] Format structured message
  - [ ] Send with trace_id

### Phase 3: Integration

- [ ] **3.1** Update vol-monitor main.rs
  - [ ] Load agent_advice config
  - [ ] Create and spawn AgentAdviceService
  - [ ] Subscribe to alert broadcast

- [ ] **3.2** Add configuration example
  - [ ] Update config.dev.toml
  - [ ] Update .env.example with ANTHROPIC_AUTH_TOKEN

- [ ] **3.3** Documentation
  - [ ] crates/vol-llm-bridge/README.md
  - [ ] docs/AGENT_ADVICE.md usage guide

### Phase 4: Testing

- [ ] **4.1** Unit tests
  - [ ] FrequencyLimiter tests
  - [ ] Prompt formatting tests

- [ ] **4.2** Integration tests
  - [ ] AgentAdviceService with mock LLM
  - [ ] TDengine query tests

- [ ] **4.3** Manual testing
  - [ ] End-to-end with real LLM
  - [ ] Verify Feishu message format
  - [ ] Verify frequency limiting

## Task Dependencies

```
1.1 → 1.2 → 2.1 → 2.2 → 2.3 → 2.4 → 3.1 → 3.2 → 4.2 → 4.3
              ↓
1.3 ──────────┘
```

## Estimated Effort

| Phase | Tasks | Estimated Time |
|-------|-------|----------------|
| Phase 1: Infrastructure | 1.1, 1.2, 1.3 | 2-3 hours |
| Phase 2: Service | 2.1, 2.2, 2.3, 2.4 | 3-4 hours |
| Phase 3: Integration | 3.1, 3.2, 3.3 | 1-2 hours |
| Phase 4: Testing | 4.1, 4.2, 4.3 | 2-3 hours |
| **Total** | | **8-12 hours** |

## Success Criteria

- [ ] AgentAdviceService compiles without errors
- [ ] All unit tests pass
- [ ] Integration tests pass with mock LLM
- [ ] Manual test: Feishu receives analysis advice
- [ ] Manual test: Frequency limiting works (5 min cooldown)
- [ ] No impact on existing alert/notification flow
