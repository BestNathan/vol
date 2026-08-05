// frontend/src/lib/event-handlers.ts
import type { UiEvent } from './protocol'
import { getDefaultStore } from 'jotai'
import {
  runCountAtom, iterationAtom, toolCallCountAtom, isRunningAtom,
  runElapsedAtom, runningAgentsAtom, runMapAtom, pendingSubmitAgentAtom,
} from '@/stores/connection'
import { agentStatusMapAtom } from '@/stores/agents'
import { conversationMapAtom, activeAgentIdAtom } from '@/stores/conversation'
import { approvalAtom, approvalPendingAtom } from '@/stores/dialogs'
import { toolCallsAtom } from '@/stores/tools'
import type { AgentConversation, ToolCallStatus } from '@/types'

const store = getDefaultStore()

// Look up owning agent for a run_id from runMap
function agentForRun(runId: string): string | undefined {
  return store.get(runMapAtom).get(runId)
}

// Helper: get or create conversation for agent. The conversation object and
// its entries array are COPIED on every update — mutating in place would leave
// conversationByAgentAtom returning the same object reference, which jotai's
// Object.is comparison treats as unchanged, so streamed updates (e.g.
// content_delta appends) would never re-render the view.
function updateConversation(agentId: string, fn: (conv: AgentConversation) => void) {
  const map = new Map(store.get(conversationMapAtom))
  const prev = map.get(agentId)
  const conv = prev
    ? { entries: [...prev.entries], autoScroll: prev.autoScroll }
    : { entries: [], autoScroll: true }
  fn(conv)
  map.set(agentId, conv)
  store.set(conversationMapAtom, map)
}

// Feed the Tools tab call history (mirrors tools_tab.rs reduce_tool_state):
// update the latest still-running entry for a tool name with its terminal
// status and duration. No-op when no matching running entry exists.
function markToolCallStatus(name: string, status: ToolCallStatus, durationMs: number | null) {
  const calls = [...store.get(toolCallsAtom)]
  for (let i = calls.length - 1; i >= 0; i--) {
    if (calls[i].toolName === name && calls[i].status === 'Running') {
      calls[i] = { ...calls[i], status, durationMs }
      break
    }
  }
  store.set(toolCallsAtom, calls)
}

let runStartTime = 0

export function handleUiEvent(event: UiEvent, runId: string) {
  switch (event.type) {
    case 'agent_start': {
      // Init run state
      store.set(runCountAtom, store.get(runCountAtom) + 1)
      store.set(iterationAtom, 0)
      store.set(toolCallCountAtom, 0)
      store.set(isRunningAtom, true)
      runStartTime = Date.now()

      // Attribute to agent
      const pendingAgent = store.get(pendingSubmitAgentAtom)
      if (pendingAgent) {
        const map = new Map(store.get(runMapAtom))
        map.set(runId, pendingAgent)
        store.set(runMapAtom, map)
        const agents = new Set(store.get(runningAgentsAtom))
        agents.add(pendingAgent)
        store.set(runningAgentsAtom, agents)
        store.set(pendingSubmitAgentAtom, null)

        // Set agent status
        const statusMap = { ...store.get(agentStatusMapAtom) }
        statusMap[pendingAgent] = { status: 'running', runId }
        store.set(agentStatusMapAtom, statusMap)

        // The UserInput was already optimistically appended by InputArea
        // — do not duplicate it here.
      }
      break
    }

    case 'agent_complete':
    case 'agent_aborted':
    case 'agent_error': {
      store.set(isRunningAtom, false)
      store.set(runElapsedAtom, Date.now() - runStartTime)

      const agentId = agentForRun(runId)
      if (agentId) {
        const statusMap = { ...store.get(agentStatusMapAtom) }
        statusMap[agentId] = { status: 'idle' }
        store.set(agentStatusMapAtom, statusMap)

        const agents = new Set(store.get(runningAgentsAtom))
        agents.delete(agentId)
        store.set(runningAgentsAtom, agents)

        const map = new Map(store.get(runMapAtom))
        map.delete(runId)
        store.set(runMapAtom, map)
      }

      if (event.type === 'agent_aborted' && agentId) {
        updateConversation(agentId, conv => {
          conv.entries.push({ type: 'Error', message: event.reason })
        })
      }
      if (event.type === 'agent_error' && agentId) {
        updateConversation(agentId, conv => {
          conv.entries.push({ type: 'Error', message: event.message })
        })
      }
      break
    }

    case 'thinking_start': {
      const agentId = agentForRun(runId) ?? store.get(activeAgentIdAtom)
      if (agentId) updateConversation(agentId, conv => {
        conv.entries.push({ type: 'Thinking', content: '' })
      })
      break
    }
    case 'thinking_delta': {
      const agentId = agentForRun(runId) ?? store.get(activeAgentIdAtom)
      if (agentId) updateConversation(agentId, conv => {
        const last = conv.entries[conv.entries.length - 1]
        if (last?.type === 'Thinking') {
          last.content += event.delta
        }
      })
      break
    }
    case 'thinking_complete': break // no-op

    case 'content_start': {
      const agentId = agentForRun(runId) ?? store.get(activeAgentIdAtom)
      if (agentId) updateConversation(agentId, conv => {
        conv.entries.push({ type: 'ContentStreaming', content: '' })
      })
      break
    }
    case 'content_delta': {
      const agentId = agentForRun(runId) ?? store.get(activeAgentIdAtom)
      if (agentId) updateConversation(agentId, conv => {
        const last = conv.entries[conv.entries.length - 1]
        if (last?.type === 'ContentStreaming') {
          last.content += event.delta
        }
      })
      break
    }
    case 'content_complete': {
      const agentId = agentForRun(runId) ?? store.get(activeAgentIdAtom)
      if (agentId) updateConversation(agentId, conv => {
        const last = conv.entries[conv.entries.length - 1]
        if (last?.type === 'ContentStreaming') {
          conv.entries[conv.entries.length - 1] = {
            type: 'AgentAnswer',
            text: event.content
          }
        } else if (event.content) {
          conv.entries.push({ type: 'AgentAnswer', text: event.content })
        }
      })
      break
    }

    case 'tool_call_begin': {
      const seq = store.get(toolCallCountAtom) + 1
      store.set(toolCallCountAtom, seq)

      // Append a Running entry to the call history (mirrors tools_tab.rs
      // reduce_tool_state: sequence, tool name, arg preview, Running).
      const calls = [...store.get(toolCallsAtom)]
      calls.push({
        sequence: seq,
        toolName: event.tool_name,
        argPreview: formatToolArgs(event.arguments),
        status: 'Running',
        durationMs: null,
      })
      store.set(toolCallsAtom, calls)

      const agentId = agentForRun(runId) ?? store.get(activeAgentIdAtom)
      if (agentId) updateConversation(agentId, conv => {
        const preview = formatToolArgs(event.arguments)
        conv.entries.push({
          type: 'ToolCall',
          toolName: event.tool_name,
          argPreview: preview,
          fullArguments: event.arguments,
        })
      })
      break
    }
    case 'tool_call_complete': {
      markToolCallStatus(event.tool_name, 'Success', event.duration_ms ?? null)

      const agentId = agentForRun(runId) ?? store.get(activeAgentIdAtom)
      if (agentId) updateConversation(agentId, conv => {
        const preview = truncatePreview(event.result, 200)
        conv.entries.push({
          type: 'ToolResult',
          toolName: event.tool_name,
          preview,
          fullResult: event.result,
          success: true,
        })
      })
      break
    }
    case 'tool_call_error': {
      markToolCallStatus(event.tool_name, 'Error', event.duration_ms ?? null)

      const agentId = agentForRun(runId) ?? store.get(activeAgentIdAtom)
      if (agentId) updateConversation(agentId, conv => {
        conv.entries.push({
          type: 'ToolResult',
          toolName: event.tool_name,
          preview: event.error,
          fullResult: event.error,
          success: false,
        })
      })
      break
    }
    case 'tool_call_skipped': {
      markToolCallStatus(event.tool_name, 'Skipped', event.duration_ms ?? null)

      const agentId = agentForRun(runId) ?? store.get(activeAgentIdAtom)
      if (agentId) updateConversation(agentId, conv => {
        conv.entries.push({
          type: 'ToolResult',
          toolName: event.tool_name,
          preview: event.reason,
          fullResult: event.reason,
          success: false,
        })
      })
      break
    }

    case 'iteration_complete': {
      store.set(iterationAtom, event.iteration)
      break
    }
    case 'max_iterations_reached': {
      const agentId = agentForRun(runId) ?? store.get(activeAgentIdAtom)
      if (agentId) updateConversation(agentId, conv => {
        conv.entries.push({
          type: 'Error',
          message: `Max iterations reached (${event.current}/${event.max}) — waiting for user decision...`
        })
      })
      break
    }
    case 'iteration_continued': {
      const agentId = agentForRun(runId) ?? store.get(activeAgentIdAtom)
      if (agentId) updateConversation(agentId, conv => {
        conv.entries.push({
          type: 'AgentAnswer',
          text: `Continuing from iteration ${event.from_iteration} (counter reset to 0)`
        })
      })
      break
    }

    case 'approval_request':
      store.set(approvalPendingAtom, true)
      // Populate the HITL dialog (ApprovalDialog). The wire event carries
      // tool_name/reason/arguments only; reqId is the run_id the event was
      // published under — the run_id agent.approve must answer.
      store.set(approvalAtom, {
        toolName: event.tool_name,
        reason: event.reason,
        arguments: event.arguments,
        reqId: runId,
      })
      break
    case 'approval_resolved':
      store.set(approvalPendingAtom, false)
      store.set(approvalAtom, { toolName: null, reason: null, arguments: null, reqId: null })
      break
    case 'ws_connected':
    case 'ws_connecting':
    case 'ws_disconnected':
    case 'ws_reconnecting':
    case 'ws_reconnect_failed':
    case 'ws_reconnected':
      // Handled at connection/approval dialog level
      break

    default: break
  }
}

// Helpers mirroring Rust state/mod.rs
export function formatToolArgs(arguments_: string): string {
  try {
    const parsed = JSON.parse(arguments_)
    if (typeof parsed === 'object' && parsed !== null && !Array.isArray(parsed)) {
      const entries = Object.entries(parsed)
      if (entries.length === 0) return ''
      if (entries.length === 1) return jsonValueToDisplay(entries[0][1])
      return entries.map(([k, v]) => `${k}=${jsonValueToDisplay(v)}`).join(', ')
    }
    return jsonValueToDisplay(parsed)
  } catch { return arguments_ }
}

function jsonValueToDisplay(v: unknown): string {
  if (typeof v === 'string') return v
  if (typeof v === 'number' || typeof v === 'boolean') return String(v)
  if (v === null) return 'null'
  const s = JSON.stringify(v)
  return s.length > 60 ? s.slice(0, 57) + '…' : s
}

export function truncatePreview(s: string, maxChars: number): string {
  if (s.length <= maxChars) return s
  return s.slice(0, maxChars) + '...'
}

export function agentEventToUiEvent(
  variant: string,
  data: Record<string, unknown>,
  runId: string,
): UiEvent | null {
  const s = (k: string) => (data[k] as string) ?? ''
  const n = (k: string) => (data[k] as number)

  switch (variant) {
    case 'AgentStart': return { type: 'agent_start', run_id: runId, input: s('input') }
    case 'AgentComplete': return { type: 'agent_complete', run_id: runId, response: s('response') }
    case 'AgentAborted': return { type: 'agent_aborted', run_id: runId, reason: s('reason') }
    case 'ThinkingStart': return { type: 'thinking_start' }
    case 'ThinkingDelta': return { type: 'thinking_delta', delta: s('delta') }
    case 'ThinkingComplete': return { type: 'thinking_complete' }
    case 'ContentStart': return { type: 'content_start' }
    case 'ContentDelta': return { type: 'content_delta', delta: s('delta') }
    case 'ContentComplete': return { type: 'content_complete', content: s('content') }
    case 'ToolCallBegin': return { type: 'tool_call_begin', tool_name: s('tool_name'), arguments: s('arguments') }
    case 'ToolCallArgumentDelta': return { type: 'tool_call_argument_delta', delta: s('delta') }
    case 'ToolCallComplete': return { type: 'tool_call_complete', tool_name: s('tool_name'), result: s('result'), duration_ms: n('duration_ms') as number | undefined }
    case 'ToolCallError': return { type: 'tool_call_error', tool_name: s('tool_name'), error: s('error'), duration_ms: n('duration_ms') as number | undefined }
    case 'ToolCallSkipped': return { type: 'tool_call_skipped', tool_name: s('tool_name'), reason: s('reason'), duration_ms: n('duration_ms') as number | undefined }
    case 'MaxIterationsReached': return { type: 'max_iterations_reached', current: (n('current_iteration') ?? 0) as number, max: (n('max_iterations') ?? 0) as number }
    case 'IterationContinued': return { type: 'iteration_continued', from_iteration: (n('from_iteration') ?? 0) as number }
    case 'IterationComplete': return { type: 'iteration_complete', iteration: (n('iteration') ?? 0) as number, final_answer: s('final_answer') || undefined }
    case 'ApprovalRequest': return { type: 'approval_request', tool_name: s('tool_name'), reason: s('reason'), arguments: s('arguments') }
    case 'ApprovalResolved': return { type: 'approval_resolved', approved: data.approved === true }
    default: return null
  }
}
