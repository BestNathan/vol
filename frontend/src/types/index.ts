// Tab routing
export type ActiveTab = 'tasks' | 'agents' | 'tools' | 'workspace' | 'skills' | 'mcp' | 'logs'
export type AgentSubTab = 'conversation' | 'sessions' | 'context' | 'tasks'
export type McpSubtab = 'servers' | 'tools' | 'resources' | 'prompts'
export type ConnectionState = 'connecting' | 'connected' | 'disconnected'
export type ServerType = 'ControlPlane' | 'DataPlane' | 'Unknown'

// Tool call status
export type ToolCallStatus = 'Running' | 'Success' | 'Error' | 'Skipped'

export interface ToolCallEntry {
  sequence: number
  toolName: string
  argPreview: string
  status: ToolCallStatus
  durationMs: number | null
}

// Conversation entries
export type ConversationEntry =
  | { type: 'UserInput'; text: string }
  | { type: 'Thinking'; content: string }
  | { type: 'ContentStreaming'; content: string }
  | { type: 'ToolCall'; toolName: string; argPreview: string; fullArguments: string }
  | { type: 'ToolResult'; toolName: string; preview: string; fullResult: string; success: boolean }
  | { type: 'AgentAnswer'; text: string }
  | { type: 'RunSummary'; iterations: number; toolCalls: number; elapsedMs: number }
  | { type: 'EntryCheckpoint'; reason: string; note: string | null; createdAt: number }
  | { type: 'Error'; message: string }
  | { type: 'RunningBanner'; runId: string }

export interface AgentConversation {
  entries: ConversationEntry[]
  autoScroll: boolean
}

// Agent list — union of the two wire shapes:
//   data-plane (data_plane/handlers/agent.rs): id, name, type, description, scope, status, current_input
//   control-plane (control_plane/handlers/client.rs): id, name, description?, status?, node_id?, ws_url?
// All fields except id/name are optional because neither shape is guaranteed across modes.
export interface AgentListEntry {
  id: string
  name: string
  type?: string
  description?: string | null
  scope?: string | null
  status?: string | null
  node_id?: string
  ws_url?: string
  current_input?: string | null
}

// Context contributors — mirrors data_plane/handlers/agent.rs context_config
// entries (and crates/vol-llm-ui/src/state/mod.rs ContributorInfoEntry).
export interface ContributorInfoEntry {
  name: string
  anchor_zone: string
  position: number
  estimated_tokens: number
  message_count: number
}
// Context snapshot message — mirrors ContextMessageEntry in the Dioxus UI state.
export interface ContextMessageEntry {
  role: string
  content: string
}

// Node types
export interface NodeLoad { running: number; queued: number }
export interface NodeListEntry {
  node_id: string
  name: string
  version: string
  status: string
  last_seen_at_ms?: number
  capability_revision: number
  load: NodeLoad
  agent_count?: number
  ws_url?: string
}

// RPC response types
export interface ConnectedInfo { server_type: ServerType; version: string; capabilities: string[] }
export interface SkillDetail {
  name: string; version: string; scope: string; description: string
  triggers: string[]; content: string; file_listing: string[]; directory: string
}
export interface SkillListEntry {
  id: string; name: string; version: string; scope: string; description: string; triggers: string[]
}
export interface McpServerInfo { name: string; status: string }
export interface McpToolInfo { server: string; name: string; description?: string; input_schema?: unknown }
export interface McpResourceInfo { server: string; name: string; uri: string; mime_type?: string; description?: string }
export interface McpResourceTemplateInfo { server: string; name: string; uri_template: string; description?: string }
export interface McpPromptInfo { server: string; name: string; description?: string; arguments?: McpPromptArgInfo[] }
export interface McpPromptArgInfo { name: string; description?: string; required: boolean }
// TaskEntry — mirrors data_plane/handlers/task.rs JSON: created_at/started_at/completed_at are
// epoch seconds (numbers); publisher/assignee/active_form serialize as null when unset.
export interface TaskEntry {
  id: number; status: string; kind: string
  publisher?: string | null; assignee?: string | null
  subject: string; description: string; active_form?: string | null
  dependencies: number[]; blocks: number[]
  created_at: number; started_at?: number | null; completed_at?: number | null
}
export interface SessionListEntry { id: string; entry_count: number; created_at: number }
export interface LogRunSummary { run_id: string; event_count: number; last_event: string; last_event_time: string }
export interface LogLine { timestamp: string; event_type: string; summary: string }
export interface FileEntry { name: string; is_dir: boolean; size: number }
export interface ProviderOption { name: string; models: string[] }

// Capability state
export interface CapabilityOverlayState {
  effective_tools: string[]; effective_skills: string[]; effective_mcp_servers: string[]
  available_tools: unknown[]; available_skills: unknown[]; available_mcp_servers: unknown[]
  base_tools: string[]; base_skills: string[]; base_mcp_servers: string[]
  loading: boolean; dirty: boolean
}

export type ToggleSavingState = { kind: 'saving' } | { kind: 'saved' } | { kind: 'error'; message: string }

// Workspace file tree — mirrors crates/vol-llm-ui/src/state/mod.rs
// WorkspaceTreeNode (recursive node; loaded/load_error describe whether the
// dir's children have been fetched via file.list).
export interface WorkspaceTreeNode {
  name: string
  path: string
  is_dir: boolean
  loaded: boolean
  load_error: boolean
  children: WorkspaceTreeNode[]
}

// Open file tab in the Workspace tab. content/error are filled asynchronously
// by file.read after the tab is added.
export interface OpenFileTab {
  path: string
  content?: string
  error?: string
}
