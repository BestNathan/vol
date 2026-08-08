import type {
  AgentListEntry,
  ConnectedInfo,
  ContextMessageEntry,
  ContributorInfoEntry,
  FileEntry,
  LogRunSummary,
  LogLine,
  McpPromptInfo,
  McpResourceInfo,
  McpResourceTemplateInfo,
  McpServerInfo,
  McpToolInfo,
  NodeListEntry,
  SessionListEntry,
  SkillDetail,
  SkillListEntry,
  TaskEntry,
} from '@/types'

// UiEvent — discriminated union keyed by "type" field (matches Rust #[serde(tag = "type")])
export type UiEvent =
  | { type: 'agent_start'; run_id: string; input: string }
  | { type: 'agent_complete'; run_id: string; response: string }
  | { type: 'agent_aborted'; run_id: string; reason: string }
  | { type: 'agent_error'; run_id: string; message: string }
  | { type: 'thinking_start' }
  | { type: 'thinking_delta'; delta: string }
  | { type: 'thinking_complete' }
  | { type: 'content_start' }
  | { type: 'content_delta'; delta: string }
  | { type: 'content_complete'; content: string }
  | { type: 'tool_call_begin'; tool_name: string; arguments: string }
  | { type: 'tool_call_argument_delta'; delta: string }
  | { type: 'tool_call_complete'; tool_name: string; result: string; duration_ms?: number }
  | { type: 'tool_call_error'; tool_name: string; error: string; duration_ms?: number }
  | { type: 'tool_call_skipped'; tool_name: string; reason: string; duration_ms?: number }
  | { type: 'max_iterations_reached'; current: number; max: number }
  | { type: 'iteration_continued'; from_iteration: number }
  | { type: 'iteration_complete'; iteration: number; final_answer?: string }
  | { type: 'approval_request'; tool_name: string; reason: string; arguments: string }
  | { type: 'approval_resolved'; approved: boolean }
  | { type: 'ws_connected' }
  | { type: 'ws_connecting' }
  | { type: 'ws_disconnected'; reason?: string }
  | { type: 'ws_reconnecting'; attempt: number; delay_secs: number }
  | { type: 'ws_reconnect_failed' }
  | { type: 'ws_reconnected' }

// AgentStreamEvent — externally tagged from server ({"VariantName": {...fields}})
export type AgentStreamEvent = Record<string, unknown>

export interface AgentEvent {
  run_id: string
  event: AgentStreamEvent
}

// RPC wire convention: every method's result is an object whose keys match the Rust
// payload struct (e.g. `agent.list` → {"agents": [...]}, `session.list` → {"sessions": [...]}),
// except `system.connected` which returns the ConnectedInfo object itself.
// Task 1.4's JsonRpcClient resolves RPC results by these envelope keys.
export interface RpcMethods {
  'agent.submit': {
    params: { input: string; target?: string }
    result: { run_id: string; response: unknown }
  }
  'agent.approve': {
    params: { run_id: string; approved: boolean; reason?: string }
    result: { run_id: string; accepted: boolean }
  }
  'agent.cancel': { params: { run_id: string }; result: { run_id: string; cancelled: boolean } }
  'agent.list': { params: { node_id?: string }; result: { agents: AgentListEntry[] } }
  'agent.status': { params: { agent_id: string }; result: { status: string; run_id?: string } }
  'agent.get_capabilities': {
    params: { agent_id: string; session_id: string }
    result: GetCapabilitiesResult
  }
  'agent.update_capabilities': {
    params: {
      agent_id: string
      session_id: string
      effective_tools: string[]
      effective_skills: string[]
      effective_mcp_servers: string[]
    }
    result: UpdateCapabilitiesResult
  }
  'agent.context_config': {
    params: { agent_id: string }
    result: { contributors: ContributorInfo[] }
  }
  'agent.context_snapshot': {
    params: { agent_id: string; contributor_name: string }
    result: { messages: ContextMessage[] }
  }
  'session.list': { params: { agent_id?: string }; result: { sessions: SessionListEntry[] } }
  'session.entries': {
    params: { session_id: string; agent_id?: string }
    result: { entries: SessionEntry[] }
  }
  'session.resume': {
    params: { session_id: string; agent_id?: string }
    result: { session_id: string; restored: boolean; entry_count: number; entries: SessionEntry[] }
  }
  'file.list': { params: { path: string }; result: { entries: FileEntry[] } }
  'file.read': { params: { path: string }; result: { content: string; metadata: unknown } }
  'tool.list': { params: object; result: { tools: ToolDef[] } }
  'tool.call': {
    params: { tool_name: string; arguments: Record<string, unknown> }
    result: { tool_name: string; result: unknown }
  }
  'skill.list': { params: object; result: { skills: SkillListEntry[] } }
  'skill.get': { params: { name: string }; result: { skill: SkillDetail; name: string } }
  'skill.refresh': { params: object; result: { discovered: number } }
  'mcp.list_servers': { params: object; result: { servers: McpServerInfo[] } }
  'mcp.list_tools': { params: { server?: string }; result: { tools: McpToolInfo[] } }
  'mcp.list_resources': { params: { server?: string }; result: { resources: McpResourceInfo[] } }
  'mcp.list_resource_templates': {
    params: { server?: string }
    result: { templates: McpResourceTemplateInfo[] }
  }
  'mcp.list_prompts': { params: object; result: { prompts: McpPromptInfo[] } }
  'mcp.read_resource': { params: { uri: string }; result: { uri: string; content: string } }
  'mcp.call_tool': {
    params: { server: string; tool_name: string; arguments: Record<string, unknown> }
    result: { tool_name: string; result: unknown }
  }
  'mcp.reconnect': { params: { server: string }; result: { reconnected: boolean } }
  'mcp.get_prompt': {
    params: { name: string; arguments?: Record<string, unknown> }
    result: { name: string; prompt: unknown }
  }
  'task.list': { params: { status?: string; assignee?: string }; result: { tasks: TaskEntry[] } }
  'task.get': { params: { task_id: number }; result: { task: TaskEntry | null } }
  'log.list': { params: object; result: { runs: LogRunSummary[] } }
  'log.read': { params: { run_id: string }; result: { entries: LogLine[] } }
  'system.connected': { params: object; result: ConnectedInfo }
  'control.node_list': { params: object; result: { nodes: NodeListEntry[] } }
  'control.node_get': { params: { node_id: string }; result: { node: NodeListEntry | null } }
  'control.capability_list': { params: { node_id?: string }; result: CapabilityListResult }
}

// Supporting types for RPC results
export interface GetCapabilitiesResult {
  effective_tools: string[]
  effective_skills: string[]
  effective_mcp_servers: string[]
  available_tools: unknown[]
  available_skills: unknown[]
  available_mcp_servers: unknown[]
  base_tools: string[]
  base_skills: string[]
  base_mcp_servers: string[]
}
export interface UpdateCapabilitiesResult {
  effective_tools: string[]
  effective_skills: string[]
  effective_mcp_servers: string[]
}
// Wire aliases of the canonical @/types entry shapes.
export type ContributorInfo = ContributorInfoEntry
export type ContextMessage = ContextMessageEntry
export interface SessionEntry {
  id: string
  session_id: string
  created_at: number
  parent_id?: string
  type: string
  data: unknown
}
export interface ToolDef {
  name: string
  description: string
  parameters?: unknown
}
// control.capability_list → {"snapshots": [CapabilitySnapshot, ...]}
export interface CapabilitySnapshot {
  node_id: string
  revision: number
  generated_at_ms?: number
  agents: AgentCapability[]
  tools: ToolCapability[]
  mcp_servers: McpServerCapability[]
  skills: SkillCapability[]
}
export interface AgentCapability {
  agent_id: string
  name: string
  description?: string | null
  status?: string | null
}
export interface ToolCapability {
  name: string
  description?: string | null
  sensitivity?: string | null
  requires_approval: boolean
}
export interface McpServerCapability {
  name: string
  status?: string | null
}
export interface SkillCapability {
  name: string
  description?: string | null
}
export interface CapabilityListResult {
  snapshots: CapabilitySnapshot[]
}
