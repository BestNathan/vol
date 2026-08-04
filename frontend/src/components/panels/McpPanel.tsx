// frontend/src/components/panels/McpPanel.tsx
// MCP panel: Servers / Tools / Resources / Prompts sub-tabs. Port of
// mcp_panel.rs. On mount (and whenever the active node changes) five list
// RPCs fire in parallel; each writes its slice into mcpStateAtom as it
// resolves, and the last one to finish clears the loading flag. Responses
// that arrive after a node switch are discarded (stale-response guard).
// Tool/resource/prompt dialogs are driven by mcpDialogAtom (stores/dialogs).
import { useCallback, useEffect, useRef, useState } from 'react'
import { useAtom, useAtomValue } from 'jotai'
import { getPanelClient } from '@/lib/panel-client'
import { mcpActiveSubtabAtom, mcpStateAtom, type McpState } from '@/stores/mcp'
import { mcpDialogAtom } from '@/stores/dialogs'
import { activeNodeIdAtom } from '@/stores/ui'
import { McpToolDialog } from '@/components/dialogs/McpToolDialog'
import { ResourceViewer } from '@/components/dialogs/ResourceViewer'
import { PromptViewer } from '@/components/dialogs/PromptViewer'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Button } from '@/components/ui/button'
import type { RpcMethods } from '@/lib/protocol'
import type {
  McpPromptInfo, McpResourceInfo, McpResourceTemplateInfo, McpServerInfo, McpSubtab, McpToolInfo,
} from '@/types'

const SUB_TABS: { id: McpSubtab; label: string }[] = [
  { id: 'servers', label: 'Servers' },
  { id: 'tools', label: 'Tools' },
  { id: 'resources', label: 'Resources' },
  { id: 'prompts', label: 'Prompts' },
]

/** Status dot color for a server status string (connected green / connecting
 * yellow / disconnected gray / anything else red). */
export function serverStatusColor(status: string): string {
  switch (status) {
    case 'connected': return '#40c040'
    case 'connecting': return '#f0c040'
    case 'disconnected': return '#888'
    default: return '#c04040'
  }
}

/** Group items by their `server` field, servers sorted by name. */
function groupByServer<T>(items: T[], serverOf: (item: T) => string): Array<[string, T[]]> {
  const map = new Map<string, T[]>()
  for (const item of items) {
    const server = serverOf(item)
    const list = map.get(server)
    if (list) list.push(item)
    else map.set(server, [item])
  }
  return [...map.entries()].sort(([a], [b]) => a.localeCompare(b))
}

function errMsg(err: unknown): string {
  return (err as { message?: string } | null)?.message ?? String(err)
}

export function McpPanel() {
  const nodeId = useAtomValue(activeNodeIdAtom)
  const [state, setState] = useAtom(mcpStateAtom)
  const [subTab, setSubTab] = useAtom(mcpActiveSubtabAtom)
  const [, setDialog] = useAtom(mcpDialogAtom)
  const [reconnecting, setReconnecting] = useState<string[]>([])

  // Live node mirror for the stale-response guard in async callbacks.
  const nodeIdRef = useRef(nodeId)
  useEffect(() => { nodeIdRef.current = nodeId }, [nodeId])

  // Fire all five MCP list calls in parallel. Each writes its slice into
  // mcpStateAtom as it resolves; the last one to finish clears loading.
  const loadAll = useCallback(async (target: string | null) => {
    if (!target) {
      setState((s) => ({
        ...s, servers: [], tools: [], resources: [], resourceTemplates: [], prompts: [],
        loading: false, error: null,
      }))
      return
    }
    setState((s) => ({ ...s, loading: true, error: null }))

    // Stale-response guard: writes are dropped once the active node no
    // longer matches the node this fetch was started for.
    const apply = (patch: Partial<McpState>) => {
      if (nodeIdRef.current !== target) return
      setState((s) => ({ ...s, ...patch }))
    }

    // "Last one clears loading" counter (mirrors the AtomicUsize in
    // mcp_panel.rs).
    let remaining = 5
    const finishOne = () => {
      remaining -= 1
      if (remaining === 0) apply({ loading: false })
    }

    const client = getPanelClient()
    client.call<RpcMethods['mcp.list_servers']['result']>('mcp.list_servers')
      .then((res) => apply({ servers: res.servers ?? [] }))
      .catch((err) => apply({ error: errMsg(err) }))
      .finally(finishOne)
    client.call<RpcMethods['mcp.list_tools']['result']>('mcp.list_tools')
      .then((res) => apply({ tools: res.tools ?? [] }))
      .catch(() => apply({ tools: [] }))
      .finally(finishOne)
    client.call<RpcMethods['mcp.list_resources']['result']>('mcp.list_resources')
      .then((res) => apply({ resources: res.resources ?? [] }))
      .catch(() => apply({ resources: [] }))
      .finally(finishOne)
    client.call<RpcMethods['mcp.list_resource_templates']['result']>('mcp.list_resource_templates')
      .then((res) => apply({ resourceTemplates: res.templates ?? [] }))
      .catch(() => apply({ resourceTemplates: [] }))
      .finally(finishOne)
    client.call<RpcMethods['mcp.list_prompts']['result']>('mcp.list_prompts')
      .then((res) => apply({ prompts: res.prompts ?? [] }))
      .catch(() => apply({ prompts: [] }))
      .finally(finishOne)
  }, [setState])

  // Fetch on mount and whenever the active node changes.
  useEffect(() => {
    void loadAll(nodeId)
  }, [loadAll, nodeId])

  // Reconnect one server, then re-fetch all five lists.
  const handleReconnect = useCallback(async (server: string) => {
    setReconnecting((prev) => (prev.includes(server) ? prev : [...prev, server]))
    try {
      const res = await getPanelClient().call<RpcMethods['mcp.reconnect']['result']>('mcp.reconnect', {
        server,
      })
      if (res.reconnected) {
        await loadAll(nodeIdRef.current)
      } else {
        setState((s) => ({ ...s, error: `Reconnect failed for '${server}'` }))
      }
    } catch (err) {
      setState((s) => ({ ...s, error: `Reconnect failed for '${server}': ${errMsg(err)}` }))
    } finally {
      setReconnecting((prev) => prev.filter((s) => s !== server))
    }
  }, [loadAll, setState])

  const openToolDialog = (tool: McpToolInfo) => {
    setDialog((d) => ({
      ...d,
      toolCallDialog: {
        server: tool.server,
        toolName: tool.name,
        argumentsJson: tool.input_schema ? JSON.stringify(tool.input_schema, null, 2) : '{}',
        inputSchema: tool.input_schema,
        result: undefined,
        error: undefined,
        loading: false,
      },
    }))
  }

  const openResourceViewer = (resource: McpResourceInfo) => {
    setDialog((d) => ({
      ...d,
      resourceViewer: { uri: resource.uri, content: undefined, error: undefined, loading: false },
    }))
  }

  const openPromptViewer = (prompt: McpPromptInfo) => {
    setDialog((d) => ({
      ...d,
      promptViewer: {
        server: prompt.server,
        promptName: prompt.name,
        argsJson: '{}',
        result: undefined,
        error: undefined,
        loading: false,
      },
    }))
  }

  if (!nodeId) {
    return (
      <div className="flex-1 overflow-y-auto p-3 flex items-center justify-center">
        <div className="text-center">
          <div className="text-[#888] text-[14px]">Select a node to view MCP data</div>
          <div className="text-[#666] text-[12px] mt-1">Select a node from the dropdown above.</div>
        </div>
      </div>
    )
  }

  if (state.loading && state.servers.length === 0 && state.error === null) {
    return (
      <div className="flex-1 flex items-center justify-center gap-2 text-[#888] text-[14px]">
        <span className="w-4 h-4 rounded-full border-2 border-[#333355] border-t-[#80a0ff] animate-spin" />
        Loading MCP data...
      </div>
    )
  }

  if (state.error !== null && state.servers.length === 0) {
    return (
      <div className="flex-1 overflow-y-auto p-3 flex items-center justify-center">
        <div className="flex flex-col items-center gap-3 text-center">
          <div className="text-[#ff6060] text-[14px]">Failed to load MCP data</div>
          <div className="text-[#888] text-[12px] max-w-[300px] break-words">{state.error}</div>
          <Button variant="outline" size="sm" onClick={() => void loadAll(nodeId)}>Retry</Button>
        </div>
      </div>
    )
  }

  return (
    <div className="flex flex-col flex-1 min-h-0 overflow-hidden">
      <Tabs
        value={subTab}
        onValueChange={(v) => setSubTab(v as McpSubtab)}
        className="flex-1 min-h-0 flex flex-col overflow-hidden"
      >
        <TabsList className="h-9 justify-start w-full gap-0 p-0 rounded-none bg-[#252540] border-b border-[#333355] flex-shrink-0 overflow-x-auto">
          {SUB_TABS.map((t) => (
            <TabsTrigger
              key={t.id}
              value={t.id}
              className="h-9 rounded-none px-3 py-1.5 text-[12px] font-semibold border-b-2 border-transparent data-[state=active]:bg-[#1a1a2e] data-[state=active]:text-[#e0e0e0] data-[state=active]:border-[#80a0ff] data-[state=active]:shadow-none"
            >
              {t.label}
            </TabsTrigger>
          ))}
        </TabsList>
        <TabsContent value="servers" className="flex-1 min-h-0 overflow-y-auto mt-0">
          <ServerList
            servers={state.servers}
            error={state.error}
            reconnecting={reconnecting}
            onReconnect={(server) => void handleReconnect(server)}
          />
        </TabsContent>
        <TabsContent value="tools" className="flex-1 min-h-0 overflow-y-auto mt-0">
          <ToolList tools={state.tools} onCall={openToolDialog} />
        </TabsContent>
        <TabsContent value="resources" className="flex-1 min-h-0 overflow-y-auto mt-0">
          <ResourceList
            resources={state.resources}
            templates={state.resourceTemplates}
            onRead={openResourceViewer}
          />
        </TabsContent>
        <TabsContent value="prompts" className="flex-1 min-h-0 overflow-y-auto mt-0">
          <PromptList prompts={state.prompts} onGet={openPromptViewer} />
        </TabsContent>
      </Tabs>
      <McpToolDialog />
      <ResourceViewer />
      <PromptViewer />
    </div>
  )
}

// --- Servers sub-tab ---------------------------------------------------------

function ServerStatusText({ status, reconnecting }: { status: string; reconnecting: boolean }) {
  if (reconnecting) {
    return (
      <span className="text-[11px] text-[#f0c040] animate-pulse flex-shrink-0 ml-2">
        Reconnecting...
      </span>
    )
  }
  return <span className="text-[11px] text-[#666] flex-shrink-0 ml-2">{status}</span>
}

function ServerReconnect({
  status,
  reconnecting,
  onReconnect,
}: {
  status: string
  reconnecting: boolean
  onReconnect: () => void
}) {
  if (status === 'connected' || status === 'connecting') return null
  if (reconnecting) {
    return <span className="text-[11px] text-[#888] animate-pulse flex-shrink-0">...</span>
  }
  return (
    <Button variant="secondary" size="sm" className="flex-shrink-0" onClick={onReconnect}>
      Reconnect
    </Button>
  )
}

function ServerList({
  servers,
  error,
  reconnecting,
  onReconnect,
}: {
  servers: McpServerInfo[]
  error: string | null
  reconnecting: string[]
  onReconnect: (server: string) => void
}) {
  if (servers.length === 0 && error === null) {
    return <div className="text-[#666] text-center p-4 text-[13px]">No MCP servers configured</div>
  }
  return (
    <div className="p-2">
      {/* Mobile: server cards */}
      <div className="sm:hidden flex flex-col gap-2">
        {servers.map((s) => (
          <div key={s.name} className="rounded-lg border border-[#333355] bg-[#20203a] p-3">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2 min-w-0">
                <span
                  className="w-2 h-2 rounded-full flex-shrink-0"
                  style={{ backgroundColor: serverStatusColor(s.status) }}
                />
                <span className="text-[13px] text-[#e0e0e0] truncate">{s.name}</span>
              </div>
              <ServerStatusText status={s.status} reconnecting={reconnecting.includes(s.name)} />
            </div>
            <div className="mt-2">
              <ServerReconnect
                status={s.status}
                reconnecting={reconnecting.includes(s.name)}
                onReconnect={() => onReconnect(s.name)}
              />
            </div>
          </div>
        ))}
      </div>
      {/* Desktop: server rows */}
      <div className="hidden sm:block font-mono text-[13px]">
        {servers.map((s) => (
          <div key={s.name} className="flex items-center justify-between py-1.5 border-b border-[#2a2a44]">
            <div className="flex items-center gap-2 min-w-0">
              <span
                className="w-2 h-2 rounded-full inline-block flex-shrink-0"
                style={{ backgroundColor: serverStatusColor(s.status) }}
              />
              <span className="text-[13px] text-[#e0e0e0] truncate">{s.name}</span>
              <ServerStatusText status={s.status} reconnecting={reconnecting.includes(s.name)} />
            </div>
            <ServerReconnect
              status={s.status}
              reconnecting={reconnecting.includes(s.name)}
              onReconnect={() => onReconnect(s.name)}
            />
          </div>
        ))}
      </div>
      {error !== null && (
        <div className="text-[#c04040] p-2 text-[12px] bg-[#2a1a1a] border border-[#c04040] rounded mt-2">
          Error: {error}
        </div>
      )}
    </div>
  )
}

// --- Tools sub-tab -----------------------------------------------------------

function ToolList({ tools, onCall }: { tools: McpToolInfo[]; onCall: (tool: McpToolInfo) => void }) {
  if (tools.length === 0) {
    return <div className="text-[#666] text-center p-4 text-[13px]">No tools available</div>
  }
  const groups = groupByServer(tools, (t) => t.server)
  return (
    <div className="p-2">
      {/* Mobile: flat tool cards */}
      <div className="sm:hidden flex flex-col gap-2">
        {tools.map((t) => (
          <div key={`${t.server}/${t.name}`} className="rounded-lg border border-[#333355] bg-[#20203a] p-3">
            <div className="flex items-center justify-between">
              <div className="min-w-0">
                <div className="truncate text-[14px] font-bold text-[#e0e0e0]">{t.name}</div>
                <div className="text-[11px] text-[#666] mt-0.5">{t.server}</div>
                {t.description && (
                  <div className="text-[11px] text-[#777] truncate mt-0.5">{t.description}</div>
                )}
              </div>
              <Button size="sm" className="flex-shrink-0 ml-2" onClick={() => onCall(t)}>Call</Button>
            </div>
          </div>
        ))}
      </div>
      {/* Desktop: grouped by server */}
      <div className="hidden sm:block font-mono text-[13px]">
        {groups.map(([server, list]) => (
          <div key={server} className="mb-2">
            <div className="text-[12px] text-[#888] font-semibold mb-1">
              {server} ({list.length} tools)
            </div>
            {list.map((t) => (
              <div key={t.name} className="flex items-center justify-between py-1 border-b border-[#2a2a44]">
                <div className="min-w-0 flex items-baseline gap-2">
                  <span className="text-[13px] text-[#e0e0e0]">{t.name}</span>
                  {t.description && <span className="text-[11px] text-[#888] truncate">{t.description}</span>}
                </div>
                <Button variant="secondary" size="sm" className="flex-shrink-0" onClick={() => onCall(t)}>
                  Call
                </Button>
              </div>
            ))}
          </div>
        ))}
      </div>
    </div>
  )
}

// --- Resources sub-tab -------------------------------------------------------

function ResourceList({
  resources,
  templates,
  onRead,
}: {
  resources: McpResourceInfo[]
  templates: McpResourceTemplateInfo[]
  onRead: (resource: McpResourceInfo) => void
}) {
  if (resources.length === 0 && templates.length === 0) {
    return <div className="text-[#666] text-center p-4 text-[13px]">No resources available</div>
  }
  const resByServer = new Map(groupByServer(resources, (r) => r.server))
  const tmpByServer = new Map(groupByServer(templates, (t) => t.server))
  const allServers = [...new Set([...resByServer.keys(), ...tmpByServer.keys()])]
    .sort((a, b) => a.localeCompare(b))

  return (
    <div className="p-2">
      {/* Mobile: flat resource + template cards */}
      <div className="sm:hidden flex flex-col gap-2">
        {resources.map((r) => (
          <div key={`${r.server}/${r.uri}`} className="rounded-lg border border-[#333355] bg-[#20203a] p-3">
            <div className="flex items-center justify-between">
              <div className="min-w-0 flex-1">
                <div className="text-[13px] text-[#e0e0e0] truncate">{r.name}</div>
                <div className="text-[11px] text-[#666] font-mono truncate mt-0.5">{r.uri}</div>
              </div>
              <Button size="sm" className="flex-shrink-0 ml-2" onClick={() => onRead(r)}>Read</Button>
            </div>
          </div>
        ))}
        {templates.map((t) => (
          <div key={`${t.server}/${t.uri_template}`} className="rounded-lg border border-[#333355] bg-[#20203a] p-3">
            <div className="flex items-center justify-between">
              <div className="min-w-0 flex-1">
                <div className="text-[13px] text-[#e0e0e0]">{t.name}</div>
                <div className="text-[11px] text-[#666] font-mono truncate mt-0.5">{t.uri_template}</div>
              </div>
              <span className="text-[10px] bg-[#2a2a44] text-[#888] px-1.5 py-0.5 rounded flex-shrink-0 ml-2">
                tmpl
              </span>
            </div>
          </div>
        ))}
      </div>
      {/* Desktop: grouped by server */}
      <div className="hidden sm:block font-mono text-[13px]">
        {allServers.map((server) => {
          const res = resByServer.get(server) ?? []
          const tmp = tmpByServer.get(server) ?? []
          return (
            <div key={server} className="mb-2">
              <div className="text-[12px] text-[#888] font-semibold mb-1">
                {server} ({res.length + tmp.length} items)
              </div>
              {res.map((r) => (
                <div key={r.uri} className="flex items-center justify-between py-1 border-b border-[#2a2a44]">
                  <div className="flex-1 min-w-0">
                    <div className="text-[13px] text-[#e0e0e0] truncate">{r.name}</div>
                    <div className="text-[11px] text-[#666] truncate">{r.uri}</div>
                  </div>
                  <Button variant="secondary" size="sm" className="flex-shrink-0 ml-2" onClick={() => onRead(r)}>
                    Read
                  </Button>
                </div>
              ))}
              {tmp.map((t) => (
                <div key={t.uri_template} className="flex items-center py-1 border-b border-[#2a2a44] text-[#888]">
                  <div className="flex-1 min-w-0">
                    <div className="text-[13px]">{t.name}</div>
                    <div className="text-[11px] text-[#666] truncate">{t.uri_template}</div>
                  </div>
                  <span className="text-[10px] bg-[#2a2a44] px-1 rounded ml-2">tmpl</span>
                </div>
              ))}
            </div>
          )
        })}
      </div>
    </div>
  )
}

// --- Prompts sub-tab ---------------------------------------------------------

function PromptList({
  prompts,
  onGet,
}: {
  prompts: McpPromptInfo[]
  onGet: (prompt: McpPromptInfo) => void
}) {
  if (prompts.length === 0) {
    return <div className="text-[#666] text-center p-4 text-[13px]">No prompts available</div>
  }
  const groups = groupByServer(prompts, (p) => p.server)
  return (
    <div className="p-2">
      {/* Mobile: prompt cards */}
      <div className="sm:hidden flex flex-col gap-2">
        {prompts.map((p) => (
          <div key={`${p.server}/${p.name}`} className="rounded-lg border border-[#333355] bg-[#20203a] p-3">
            <div className="flex items-center justify-between">
              <div className="min-w-0">
                <div className="truncate text-[14px] font-bold text-[#e0e0e0]">{p.name}</div>
                <div className="text-[11px] text-[#666] mt-0.5">{p.server}</div>
                {p.description && (
                  <div className="text-[11px] text-[#777] truncate mt-0.5">{p.description}</div>
                )}
              </div>
              <Button size="sm" className="flex-shrink-0 ml-2" onClick={() => onGet(p)}>Get</Button>
            </div>
          </div>
        ))}
      </div>
      {/* Desktop: grouped by server */}
      <div className="hidden sm:block font-mono text-[13px]">
        {groups.map(([server, list]) => (
          <div key={server} className="mb-2">
            <div className="text-[12px] text-[#888] font-semibold mb-1">
              {server} ({list.length} prompts)
            </div>
            {list.map((p) => (
              <div key={p.name} className="flex items-center justify-between py-1 border-b border-[#2a2a44]">
                <div className="min-w-0 flex items-baseline gap-2">
                  <span className="text-[13px] text-[#e0e0e0]">{p.name}</span>
                  {p.description && <span className="text-[11px] text-[#888] truncate">{p.description}</span>}
                </div>
                <Button variant="secondary" size="sm" className="flex-shrink-0" onClick={() => onGet(p)}>
                  Get
                </Button>
              </div>
            ))}
          </div>
        ))}
      </div>
    </div>
  )
}
