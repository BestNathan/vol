// frontend/src/lib/dp-pool.ts
import { JsonRpcClient } from './jsonrpc-client'
import type { AgentEvent } from './protocol'

export interface DpConnection {
  client: JsonRpcClient
  nodeId: string
  wsUrl: string
  agentIds: string[]
}

export class DpConnectionPool {
  // Named `entries` (not `connections`) because the public `connections()`
  // method below shares the name — an instance field would shadow it.
  private entries = new Map<string, DpConnection>()
  private eventHandler: ((event: AgentEvent) => void) | null = null

  /** Register a handler for agent events on all current and future DP connections. */
  setEventHandler(handler: (event: AgentEvent) => void): void {
    this.eventHandler = handler
    // Register on existing connections
    for (const entry of this.entries.values()) {
      entry.client.onEvent(handler)
    }
  }

  getOrCreate(nodeId: string, wsUrl: string, agentIds: string[] = []): JsonRpcClient {
    let entry = this.entries.get(nodeId)
    if (!entry) {
      const client = new JsonRpcClient(wsUrl)
      // Register the shared event handler so agent events from DP nodes
      // are relayed to the UI (critical for CP mode where events don't
      // come through the main connection).
      if (this.eventHandler) {
        client.onEvent(this.eventHandler)
      }
      entry = { client, nodeId, wsUrl, agentIds }
      this.entries.set(nodeId, entry)
    }
    return entry.client
  }

  get(nodeId: string): JsonRpcClient | undefined {
    return this.entries.get(nodeId)?.client
  }

  connections(): DpConnection[] {
    return Array.from(this.entries.values())
  }
}

// Shared module-level pool (mirrors getPanelClient in panel-client.ts): keeps
// one direct DP WebSocket per node across all components — NodesDropdown node
// selection, NodeDetailPanel agent fetch, etc.
export const dpPool = new DpConnectionPool()
