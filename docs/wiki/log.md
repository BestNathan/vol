# Change Log

## [2026-05-06] ingest | Clarifying Requirements Subagent Review
- Created sources: [[clarifying-requirements-subagent-review]]
- Created concepts: [[subagent-review-pattern]], [[clarifying-requirements-workflow]]
- Updated concepts: [[skill-system]], [[human-in-the-loop]]
- Cross-references added: 4

## [2026-05-05] update | HTTP Transport improvements and tests
- Updated concepts: [[http-transport]], [[connection-trait]], [[connection-holder]]
- Updated entity: [[vol-llm-agent-channel-crate]]
- Changes: SSE stream termination (drop event_tx vs 100ms sleep), holder detach on stream end, 409 for concurrent SSE requests, simplified HttpEventConnection, 5 tests added

## [2026-05-05] ingest | HTTP Transport Implementation
- Created sources: [[http-transport-impl]]
- Created concepts: [[http-transport]], [[connection-trait]], [[connection-holder]], [[agent-dispatcher]]
- Created entity: [[vol-llm-agent-channel-crate]]
- Cross-references added: 6+

## [2026-05-04] ingest | Agent Component Documentation (tools, skills, session, context)
- Created sources: [[agent-tool-design]], [[skills-as-react-native]], [[session-ssot-redesign]]
- Created concepts: [[skill-system]], [[session-as-ssot]], [[run-context]], [[context-builder]], [[session-contributor]], [[session-compression]], [[plugin-context-migration]], [[context-error]], [[tool-trait]], [[tool-context]]
- Created entity: [[vol-session]]
- Updated concepts: [[tool-registry]], [[agent-plugin-system]], [[react-pattern]], [[agent-builder-pattern]], [[agent-event-stream]], [[vol-llm-agent-crate]]
- Cross-references added: 15+

## [2026-05-04] ingest | ReAct Agent Documentation
- Created: [[react-agent-docs]]
- Created concepts: [[react-pattern]], [[agent-plugin-system]], [[plugin-actions]], [[built-in-plugins]], [[agent-event-stream]], [[agent-builder-pattern]], [[tool-registry]], [[agent-observability]], [[semantic-caching]], [[human-in-the-loop]], [[retry-with-backoff]], [[rate-limiting]]
- Created entities: [[vol-llm-agent-crate]], [[vol-llm-agents-crate]], [[vol-llm-core-crate]], [[vol-llm-tool-crate]], [[vol-llm-provider-crate]], [[tdengine]], [[dashscope]]
- Cross-references added: 12
