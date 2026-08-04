// frontend/tests/unit/event-handlers.test.ts
import { describe, it, expect } from 'vitest'
import { agentEventToUiEvent } from '@/lib/event-handlers'

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
