// frontend/src/lib/dp-pool.ts
import { JsonRpcClient } from './jsonrpc-client'

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

  getOrCreate(nodeId: string, wsUrl: string, agentIds: string[] = []): JsonRpcClient {
    let entry = this.entries.get(nodeId)
    if (!entry) {
      const client = new JsonRpcClient(wsUrl)
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
