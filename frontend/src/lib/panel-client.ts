// Shared RPC client reference for panel components. Set by App on mount so
// all panels reuse the same WebSocket connection (and its reconnect loop).
import { JsonRpcClient } from './jsonrpc-client'

let panelClient: JsonRpcClient | null = null

export function setPanelClient(client: JsonRpcClient): void {
  panelClient = client
}

export function getPanelClient(): JsonRpcClient {
  if (!panelClient) {
    throw new Error('Panel client not initialized — App must call setPanelClient on mount')
  }
  return panelClient
}
