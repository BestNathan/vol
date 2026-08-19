// @ts-check
// Playwright e2e helpers: mock the vol-agent-server JSON-RPC backend at the
// WebSocket layer so the real React app runs against the Vite dev server
// without a live backend on :3001.
//
// The app connects every client (main event client, panel RPC client, per-node
// DP client) to ws://localhost:3001/ws (deriveWsUrl). We intercept that URL
// with page.routeWebSocket and answer JSON-RPC requests from a small state
// table; agent.event notifications are pushed through the first connection
// that subscribed (the main event client in App.tsx).

/** Mock data-plane node returned by control.node_list. */
export const MOCK_NODE = {
  node_id: 'node-1',
  name: 'Test Node',
  version: '0.0.0-test',
  status: 'online',
  capability_revision: 1,
  load: { running: 0, queued: 0 },
  agent_count: 1,
  ws_url: 'ws://localhost:3001/ws',
}

/** Mock agent returned by agent.list. */
export const MOCK_AGENT = {
  id: 'agent-1',
  name: 'Test Agent',
  type: 'test',
  description: 'Playwright mock agent',
  scope: 'repo',
  status: 'idle',
}

/** Default agent.get_capabilities payload (bash base tool, one skill, one MCP). */
export function defaultCapabilities(overrides = {}) {
  return {
    effective_tools: [],
    effective_skills: [],
    effective_mcp_servers: [],
    available_tools: [
      { name: 'bash', description: 'Run shell commands' },
      { name: 'read_file', description: 'Read a file' },
    ],
    available_skills: [{ name: 'explore', description: 'Explore the codebase' }],
    available_mcp_servers: [{ name: 'filesystem', description: 'Filesystem MCP server' }],
    base_tools: ['bash'],
    base_skills: [],
    base_mcp_servers: [],
    ...overrides,
  }
}

/**
 * Intercept the app's WebSocket and answer JSON-RPC requests.
 * `handlers` optionally maps extra method names to result-producing functions
 * (e.g. `{ 'file.list': (params) => ({ entries: [...] }) }`); unknown methods
 * still resolve harmlessly to `{}`.
 * @returns {{ pushEvent: (agentEvent: unknown) => void, capabilities: object }}
 */
export async function installMockBackend(page, { capabilities, handlers } = {}) {
  const caps = capabilities ?? defaultCapabilities()
  const extra = handlers ?? {}
  // Event channel for agent.event notifications. App.tsx is the only caller of
  // system.connected (it is not called by the panel/DP clients), and React
  // StrictMode double-mounts App.tsx's effect — the app listens on the LAST
  // created main client — so last system.connected wins.
  let eventWs = null

  await page.routeWebSocket('ws://localhost:3001/ws', (ws) => {
    ws.onMessage((message) => {
      let msg
      try {
        msg = JSON.parse(String(message))
      } catch {
        return
      }
      if (msg.id == null) return // client -> server notification, nothing to answer

      let result
      switch (msg.method) {
        case 'system.connected':
          eventWs = ws
          result = { server_type: 'ControlPlane', version: '0.0.0-test', capabilities: [] }
          break
        case 'agent.subscribe':
          result = {}
          break
        case 'control.node_list':
          result = { nodes: [MOCK_NODE] }
          break
        case 'agent.list':
          result = { agents: [MOCK_AGENT] }
          break
        case 'agent.status':
          result = { status: 'idle' }
          break
        case 'agent.get_capabilities':
          result = caps
          break
        case 'agent.update_capabilities':
          caps.effective_tools = msg.params?.effective_tools ?? caps.effective_tools
          caps.effective_skills = msg.params?.effective_skills ?? caps.effective_skills
          caps.effective_mcp_servers = msg.params?.effective_mcp_servers ?? caps.effective_mcp_servers
          result = {
            effective_tools: caps.effective_tools,
            effective_skills: caps.effective_skills,
            effective_mcp_servers: caps.effective_mcp_servers,
          }
          break
        default:
          // Custom handlers first, then harmless `{}` for everything else
          // (tool.list, task.list, log.list, ...).
          result = extra[msg.method] ? extra[msg.method](msg.params, msg.id) : {}
      }
      ws.send(JSON.stringify({ id: msg.id, result }))
    })
  })

  return {
    /** Send an agent.event notification through the app's event channel. */
    pushEvent(agentEvent) {
      if (!eventWs) throw new Error('mock backend: no subscribed event channel yet')
      eventWs.send(JSON.stringify({ method: 'agent.event', params: agentEvent }))
    },
    capabilities: caps,
  }
}

/**
 * Load the app and reach a state where an agent is selected:
 * node selector -> mock node -> agent card. Assumes installMockBackend ran.
 *
 * The NodesDropdown auto-selects the first online node with a ws_url, so by
 * the time the trigger shows the node name ("▾ Test Node") the DP pool is
 * already connected — the dropdown no longer needs manual interaction.
 */
export async function selectAgent(page) {
  // 'domcontentloaded' avoids racing the Vite dep pre-bundling; locators below
  // auto-wait for the app to render.
  await page.goto('/', { waitUntil: 'domcontentloaded' })
  await page.getByRole('button', { name: /▾ Test Node/ }).waitFor()
  await page.getByRole('button', { name: /Test Agent/ }).click()
}
