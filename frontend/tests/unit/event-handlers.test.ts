// frontend/tests/unit/event-handlers.test.ts
import { beforeEach, describe, it, expect } from 'vitest'
import { getDefaultStore } from 'jotai'
import { agentEventToUiEvent, handleUiEvent } from '@/lib/event-handlers'
import { toolCallsAtom } from '@/stores/tools'
import { approvalAtom, approvalPendingAtom } from '@/stores/dialogs'

describe('agentEventToUiEvent', () => {
  it('maps ApprovalRequest to the approval_request UiEvent', () => {
    expect(agentEventToUiEvent('ApprovalRequest', {
      tool_name: 'bash',
      reason: 'sensitive command',
      arguments: '{"cmd":"ls"}',
    }, 'run-1')).toEqual({
      type: 'approval_request',
      tool_name: 'bash',
      reason: 'sensitive command',
      arguments: '{"cmd":"ls"}',
    })
  })

  it('maps ApprovalResolved with the approved flag', () => {
    expect(agentEventToUiEvent('ApprovalResolved', { approved: true }, 'run-1'))
      .toEqual({ type: 'approval_resolved', approved: true })
    expect(agentEventToUiEvent('ApprovalResolved', { approved: false }, 'run-1'))
      .toEqual({ type: 'approval_resolved', approved: false })
  })

  it('returns null for unmapped variants', () => {
    expect(agentEventToUiEvent('UnknownVariant', {}, 'run-1')).toBeNull()
  })
})

// handleUiEvent → toolCallsAtom (Tools tab call history; mirrors tools_tab.rs
// reduce_tool_state). Conversation writes are inert here: no runMap entries
// and no active agent.
describe('handleUiEvent → toolCallsAtom', () => {
  const store = getDefaultStore()

  beforeEach(() => {
    store.set(toolCallsAtom, [])
  })

  it('appends a Running entry on tool_call_begin with seq and arg preview', () => {
    handleUiEvent(
      { type: 'tool_call_begin', tool_call_id: 'c1', tool_name: 'bash', arguments: '{"command":"ls"}' },
      'run-1'
    )
    expect(store.get(toolCallsAtom)).toEqual([
      { sequence: 1, toolName: 'bash', argPreview: 'ls', status: 'Running', durationMs: null },
    ])
  })

  it('marks the matching Running entry Success with duration on tool_call_complete', () => {
    store.set(toolCallsAtom, [
      { sequence: 1, toolName: 'bash', argPreview: 'ls', status: 'Running', durationMs: null },
    ])
    handleUiEvent(
      { type: 'tool_call_complete', tool_call_id: 'c1', tool_name: 'bash', result: 'ok', duration_ms: 12 },
      'run-1'
    )
    expect(store.get(toolCallsAtom)).toEqual([
      { sequence: 1, toolName: 'bash', argPreview: 'ls', status: 'Success', durationMs: 12 },
    ])
  })

  it('marks Error on tool_call_error and Skipped on tool_call_skipped', () => {
    store.set(toolCallsAtom, [
      { sequence: 1, toolName: 'bash', argPreview: 'ls', status: 'Running', durationMs: null },
      { sequence: 2, toolName: 'read_file', argPreview: 'x', status: 'Running', durationMs: null },
    ])
    handleUiEvent(
      { type: 'tool_call_error', tool_call_id: 'c1', tool_name: 'bash', error: 'boom', duration_ms: 3 },
      'run-1'
    )
    handleUiEvent(
      { type: 'tool_call_skipped', tool_call_id: 'c2', tool_name: 'read_file', reason: 'no', duration_ms: 4 },
      'run-1'
    )
    expect(store.get(toolCallsAtom)).toEqual([
      { sequence: 1, toolName: 'bash', argPreview: 'ls', status: 'Error', durationMs: 3 },
      { sequence: 2, toolName: 'read_file', argPreview: 'x', status: 'Skipped', durationMs: 4 },
    ])
  })

  it('updates only the latest Running entry when the same tool runs twice', () => {
    store.set(toolCallsAtom, [
      { sequence: 1, toolName: 'bash', argPreview: 'ls', status: 'Running', durationMs: null },
      { sequence: 2, toolName: 'bash', argPreview: 'pwd', status: 'Running', durationMs: null },
    ])
    handleUiEvent(
      { type: 'tool_call_complete', tool_call_id: 'c2', tool_name: 'bash', result: 'ok', duration_ms: 7 },
      'run-1'
    )
    const calls = store.get(toolCallsAtom)
    expect(calls[0]).toEqual({
      sequence: 1, toolName: 'bash', argPreview: 'ls', status: 'Running', durationMs: null,
    })
    expect(calls[1]).toEqual({
      sequence: 2, toolName: 'bash', argPreview: 'pwd', status: 'Success', durationMs: 7,
    })
  })

  it('leaves entries untouched when no Running entry matches', () => {
    store.set(toolCallsAtom, [
      { sequence: 1, toolName: 'bash', argPreview: 'ls', status: 'Success', durationMs: 5 },
    ])
    handleUiEvent(
      { type: 'tool_call_complete', tool_call_id: 'c1', tool_name: 'bash', result: 'ok', duration_ms: 12 },
      'run-1'
    )
    expect(store.get(toolCallsAtom)).toEqual([
      { sequence: 1, toolName: 'bash', argPreview: 'ls', status: 'Success', durationMs: 5 },
    ])
  })
})

// handleUiEvent → approval state (HITL): approval_request populates
// approvalAtom — the source for ApprovalDialog — with the run_id as reqId,
// and approval_resolved clears it alongside the pending banner flag.
describe('handleUiEvent → approval state', () => {
  const store = getDefaultStore()

  beforeEach(() => {
    store.set(approvalPendingAtom, false)
    store.set(approvalAtom, { toolName: null, reason: null, arguments: null, reqId: null })
  })

  it('populates approvalAtom on approval_request with the run_id as reqId', () => {
    handleUiEvent(
      { type: 'approval_request', tool_name: 'bash', reason: 'sensitive command', arguments: '{"cmd":"ls"}' },
      'run-42'
    )
    expect(store.get(approvalPendingAtom)).toBe(true)
    expect(store.get(approvalAtom)).toEqual({
      toolName: 'bash',
      reason: 'sensitive command',
      arguments: '{"cmd":"ls"}',
      reqId: 'run-42',
    })
  })

  it('clears approvalAtom and the pending flag on approval_resolved', () => {
    store.set(approvalPendingAtom, true)
    store.set(approvalAtom, {
      toolName: 'bash', reason: 'r', arguments: 'a', reqId: 'run-42',
    })
    handleUiEvent({ type: 'approval_resolved', approved: true }, 'run-42')
    expect(store.get(approvalPendingAtom)).toBe(false)
    expect(store.get(approvalAtom)).toEqual({
      toolName: null, reason: null, arguments: null, reqId: null,
    })
  })
})
