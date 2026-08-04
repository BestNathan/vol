import type {
  AgentListEntry, ConnectedInfo, FileEntry, LogRunSummary, LogLine,
  McpPromptInfo, McpResourceInfo, McpResourceTemplateInfo, McpServerInfo, McpToolInfo,
  NodeListEntry, SessionListEntry, SkillDetail, SkillListEntry, TaskEntry
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
  | { type: 'ws_connected' } | { type: 'ws_connecting' }
  | { type: 'ws_disconnected'; reason?: string }
  | { type: 'ws_reconnecting'; attempt: number; delay_secs: number }
  | { type: 'ws_reconnect_failed' } | { type: 'ws_reconnected' }

// AgentStreamEvent — externally tagged from server ({"VariantName": {...fields}})
export type AgentStreamEvent = Record<string, unknown>

export interface AgentEvent {
  run_id: string
  event: AgentStreamEvent
}

// All RPC method signatures with parameter and return types
export interface RpcMethods {
  'agent.submit': { params: { input: string; target?: string }; result: string }
  'agent.approve': { params: { req_id: string; approved: boolean; reason?: string }; result: null }
  'agent.cancel': { params: { run_id: string }; result: null }
  'agent.list': { params: { node_id?: string }; result: AgentListEntry[] }
  'agent.status': { params: { agent_id: string }; result: { status: string; run_id?: string } }
  'agent.get_capabilities': { params: { agent_id: string; session_id: string }; result: GetCapabilitiesResult }
  'agent.update_capabilities': { params: { agent_id: string; session_id: string; effective_tools: string[]; effective_skills: string[]; effective_mcp_servers: string[] }; result: UpdateCapabilitiesResult }
  'agent.context_config': { params: { agent_id: string }; result: { contributors: ContributorInfo[] } }
  'agent.context_snapshot': { params: { agent_id: string; contributor: string }; result: { messages: ContextMessage[] } }
  'session.list': { params: { agent_id?: string }; result: SessionListEntry[] }
  'session.entries': { params: { session_id: string }; result: SessionEntry[] }
  'session.resume': { params: { session_id: string; agent_id?: string }; result: null }
  'file.list': { params: { path: string }; result: FileEntry[] }
  'file.read': { params: { path: string }; result: string }
  'tool.list': { params: {}; result: ToolDef[] }
  'tool.call': { params: { tool_name: string; arguments: Record<string, unknown> }; result: string }
  'skill.list': { params: {}; result: SkillListEntry[] }
  'skill.get': { params: { name: string }; result: SkillDetail }
  'skill.refresh': { params: {}; result: null }
  'mcp.list_servers': { params: {}; result: McpServerInfo[] }
  'mcp.list_tools': { params: { server?: string }; result: McpToolInfo[] }
  'mcp.list_resources': { params: {}; result: McpResourceInfo[] }
  'mcp.list_resource_templates': { params: {}; result: McpResourceTemplateInfo[] }
  'mcp.list_prompts': { params: {}; result: McpPromptInfo[] }
  'mcp.read_resource': { params: { uri: string }; result: string }
  'mcp.call_tool': { params: { server: string; tool_name: string; arguments: Record<string, unknown> }; result: string }
  'mcp.reconnect': { params: { server: string }; result: null }
  'mcp.get_prompt': { params: { server: string; prompt_name: string; arguments: Record<string, unknown> }; result: string }
  'task.list': { params: { status?: string; assignee?: string }; result: TaskEntry[] }
  'task.get': { params: { task_id: number }; result: TaskEntry }
  'log.list': { params: {}; result: LogRunSummary[] }
  'log.read': { params: { run_id: string }; result: LogLine[] }
  'system.connected': { params: {}; result: ConnectedInfo }
  'control.node_list': { params: {}; result: NodeListEntry[] }
  'control.node_get': { params: { node_id: string }; result: NodeListEntry }
  'control.capability_list': { params: { node_id: string }; result: CapabilityListResult }
}

// Supporting types for RPC results
export interface GetCapabilitiesResult {
  effective_tools: string[]; effective_skills: string[]; effective_mcp_servers: string[]
  available_tools: unknown[]; available_skills: unknown[]; available_mcp_servers: unknown[]
  base_tools: string[]; base_skills: string[]; base_mcp_servers: string[]
  providers?: { name: string; models: string[] }[]
  selected_provider?: string; selected_model?: string
}
export interface UpdateCapabilitiesResult extends GetCapabilitiesResult {}
export interface ContributorInfo { name: string; anchor_zone: string; position: number; estimated_tokens: number; message_count: number }
export interface ContextMessage { role: string; content: string }
export interface SessionEntry { id: string; session_id: string; created_at: string; parent_id?: string; type: string; data: unknown }
export interface ToolDef { name: string; description: string; parameters?: unknown }
export interface CapabilityListResult { node_id: string; revision: number; agents: unknown[]; tools: unknown[]; mcp_servers: unknown[]; skills: unknown[] }
