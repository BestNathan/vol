// frontend/src/lib/panel-client.ts
// Shared RPC-only JsonRpcClient for panel components (AgentsPanel, InputArea,
// CapabilityBar, CapabilityDrawer). A single module-level instance keeps one
// WebSocket connection for panel RPC. autoSubscribe is off because App.tsx's
// client already subscribes to the event stream and dispatches events into
// stores; this client is used purely for call().
import { JsonRpcClient } from './jsonrpc-client'
import { deriveWsUrl } from './ws-url'

let panelClient: JsonRpcClient | null = null

export function getPanelClient(): JsonRpcClient {
  if (!panelClient) {
    panelClient = new JsonRpcClient(deriveWsUrl(), { autoSubscribe: false })
  }
  return panelClient
}
