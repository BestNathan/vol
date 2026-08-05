// frontend/src/lib/panel-client.ts
//
// Connection routing layer. Mirrors the Dioxus AppState::agent_client()
// pattern: in DataPlane mode the main connection also serves agent ops;
// in ControlPlane mode agent ops are routed through per-node DP pool
// connections while CP-only ops (node_list, node_get, capability_list,
// system.connected) stay on the control-plane connection.
//
// Initialised once by App on mount. Every panel imports { getControlClient,
// getAgentClient, getDpPool } and never creates its own JsonRpcClient.

import { JsonRpcClient } from './jsonrpc-client'
import { DpConnectionPool, dpPool } from './dp-pool'
import { getDefaultStore } from 'jotai'
import { serverModeAtom } from '@/stores/connection'
import { activeNodeIdAtom } from '@/stores/ui'
import type { ServerType } from '@/types'

// Re-export for convenience so panels can import from one place
export { dpPool } from './dp-pool'

/**
 * @deprecated Use getAgentClient() — same function, clearer name.
 * Kept for backward compat with existing panel imports.
 */
export const getPanelClient = getAgentClient

// ── Internal state (set once by App) ─────────────────────────────────────

let controlClient: JsonRpcClient | null = null
let pool: DpConnectionPool = dpPool

// ── Public API ───────────────────────────────────────────────────────────

/** Called once by App on mount. */
export function initClients(mainClient: JsonRpcClient, dpConnectionPool?: DpConnectionPool): void {
  controlClient = mainClient
  if (dpConnectionPool) pool = dpConnectionPool
}

/**
 * The control-plane connection — used ONLY for control-plane protocol
 * (control.node_list, control.node_get, control.capability_list,
 * system.connected). Never use this for agent-level RPCs when in CP mode.
 */
export function getControlClient(): JsonRpcClient {
  if (!controlClient) {
    throw new Error('Clients not initialised — App must call initClients on mount')
  }
  return controlClient
}

/**
 * The connection for agent-level operations (agent.*, tool.*, skill.*, mcp.*,
 * session.*, task.*, file.*, log.*).
 *
 * - DataPlane mode → the main connection IS the DP connection — use it directly.
 * - ControlPlane mode → route through the active DP node's pool connection.
 *   Falls back to the control-plane client if no node is selected yet (the
 *   caller will get an RPC error, which is the correct behaviour).
 */
export function getAgentClient(): JsonRpcClient {
  if (!controlClient) {
    throw new Error('Clients not initialised — App must call initClients on mount')
  }

  const store = getDefaultStore()
  const serverMode: ServerType = store.get(serverModeAtom)
  const activeNodeId: string | null = store.get(activeNodeIdAtom)

  if (serverMode === 'ControlPlane') {
    if (!activeNodeId) {
      throw new Error('No data-plane node selected — select a node from the dropdown')
    }
    const dpClient = pool.get(activeNodeId)
    if (dpClient) return dpClient
    throw new Error(`Data-plane connection not available for node "${activeNodeId}" — the node may be offline`)
  }

  // DataPlane mode: the main connection IS the DP connection.
  return controlClient
}

/**
 * The per-node data-plane connection pool. Used by NodesDropdown /
 * AgentsPanel to create DP connections when a node is selected.
 */
export function getDpPool(): DpConnectionPool {
  return pool
}
