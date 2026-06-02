---
type: source
source_type: report
date: 2026-05-29
ingested: 2026-05-29
tags: [cleanup, web, json-rpc, crate-removal]
---

# Remove vol-agent-manager and Legacy Frontend

**Authors/Creators:** Claude Code session
**Date:** 2026-05-29
**Link:** workspace cleanup

## TL;DR

The obsolete `vol-agent-manager` crate, its Docker/Kubernetes deployment artifacts, and the legacy React `frontend/` app were removed. The active web backend remains `make web-backend`, which runs `vol-llm-agent-channel`'s `jsonrpc_agent_service` example on port 3001.

## Key Takeaways

- `vol-agent-manager` was self-contained and unused by the current web backend.
- The legacy `frontend/` app proxied to `vol-agent-manager:8080` and was removed with the manager.
- `crates/vol-llm-ui` remains the active Dioxus/WASM web frontend.
- `make web-backend` remains the supported JSON-RPC backend startup path.

## Detailed Summary

The workspace member `crates/vol-agent-manager` was removed from the root Cargo workspace and the crate directory was deleted. Manager-specific deployment artifacts (`Dockerfile.agent-manager`, `k8s/deploy-agent-manager.sh`, `k8s/deployment-agent-manager.yaml`) were deleted because they only built or deployed the removed service.

The legacy React `frontend/` directory was also deleted because it referenced `vol-agent-manager` in its package metadata and Nginx proxy configuration. Current web development uses `crates/vol-llm-ui` and the Makefile web targets.

## Entities Mentioned

- [[vol-llm-ui-crate]]: active Dioxus/WASM frontend after cleanup.
- [[vol-llm-agent-channel-crate]]: owns the current JSON-RPC backend example used by `make web-backend`.

## Concepts Covered

- [[json-rpc-websocket]]: current backend transport path remains JSON-RPC over WebSocket.

## Notes

Historical docs may still mention `vol-agent-manager` as part of past architecture, but active workspace/build/deployment references should not.
