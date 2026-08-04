// frontend/src/stores/dialogs.ts
// Dialog/overlay state: tool-approval (HITL), MCP tool-call/resource/prompt
// dialogs, skill detail dialog, and the debug panel. `approvalPendingAtom`
// (the InputArea banner flag) lives here too.
import { atom } from 'jotai'

// True while a tool approval request is pending (set on the approval_request
// event, cleared on approval_resolved). Consumed by InputArea to show the
// "Tool approval pending" banner in place of the textarea.
export const approvalPendingAtom = atom(false)

export interface ApprovalState { toolName: string | null; reason: string | null; arguments: string | null; reqId: string | null }
export const approvalAtom = atom<ApprovalState>({ toolName: null, reason: null, arguments: null, reqId: null })

export interface McpDialogState {
  toolCallDialog: { server: string; toolName: string; argumentsJson: string; inputSchema?: unknown; result?: string; error?: string; loading: boolean } | null
  resourceViewer: { uri: string; content?: string; error?: string; loading: boolean } | null
  promptViewer: { server: string; promptName: string; argsJson: string; result?: string; error?: string; loading: boolean } | null
}
export const mcpDialogAtom = atom<McpDialogState>({ toolCallDialog: null, resourceViewer: null, promptViewer: null })

export interface SkillDialogState { open: boolean; skill: import('@/types').SkillDetail | null; loading: boolean }
export const skillDialogAtom = atom<SkillDialogState>({ open: false, skill: null, loading: false })

export interface DebugMessage { direction: 'in' | 'out'; method: string; payload: string; elapsedMs: number }
export const debugPanelAtom = atom<{ open: boolean; messages: DebugMessage[] }>({ open: false, messages: [] })

// capability.ts already exists from Task 2.5 — these atoms already exist:
// capOverlayAtom, drawerOpenAtom, drawerSearchAtom, savingStatesAtom, selectedToolsAtom, selectedSkillsAtom, selectedMcpsAtom
