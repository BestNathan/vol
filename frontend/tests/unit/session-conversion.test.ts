// frontend/tests/unit/session-conversion.test.ts
import { describe, it, expect } from 'vitest'
import { sessionEntriesToConversation } from '@/components/panels/AgentsPanel'
import type { SessionEntry } from '@/lib/protocol'

describe('sessionEntriesToConversation', () => {
  it('converts user, assistant (thinking + tool_calls), and tool messages', () => {
    const entries = [
      { id: '1', session_id: 's', created_at: 1, type: 'message', data: { message: { message: { role: 'user', content: 'hello' } } } },
      {
        id: '2', session_id: 's', created_at: 2, type: 'message',
        data: {
          message: {
            message: {
              role: 'assistant',
              content: [{ type: 'text', text: 'hi there' }],
              thinking: 'hmm',
              tool_calls: [{ name: 'bash', arguments: '{"cmd":"ls"}' }],
            },
          },
        },
      },
      { id: '3', session_id: 's', created_at: 3, type: 'message', data: { message: { message: { role: 'tool', name: 'bash', content: 'file1 file2' } } } },
    ] as SessionEntry[]

    expect(sessionEntriesToConversation(entries)).toEqual([
      { type: 'UserInput', text: 'hello' },
      { type: 'Thinking', content: 'hmm' },
      { type: 'ToolCall', toolName: 'bash', argPreview: 'ls', fullArguments: '{"cmd":"ls"}' },
      { type: 'AgentAnswer', text: 'hi there' },
      { type: 'ToolResult', toolName: 'bash', preview: 'file1 file2', fullResult: 'file1 file2', success: true },
    ])
  })

  it('handles plain-string content and missing optional fields', () => {
    const entries = [
      { id: '1', session_id: 's', created_at: 1, type: 'message', data: { message: { message: { role: 'assistant', content: 'plain answer' } } } },
      { id: '2', session_id: 's', created_at: 2, type: 'message', data: { message: { message: { role: 'user' } } } },
    ] as SessionEntry[]

    expect(sessionEntriesToConversation(entries)).toEqual([
      { type: 'AgentAnswer', text: 'plain answer' },
      { type: 'UserInput', text: '' },
    ])
  })

  it('converts checkpoint and summary entries', () => {
    const entries = [
      { id: '1', session_id: 's', created_at: 5, type: 'checkpoint', data: { checkpoint: { reason: 'progress', note: 'keep going' } } },
      { id: '2', session_id: 's', created_at: 6, type: 'checkpoint', data: { checkpoint: { reason: 'milestone' } } },
      { id: '3', session_id: 's', created_at: 7, type: 'summary', data: {} },
    ] as SessionEntry[]

    expect(sessionEntriesToConversation(entries)).toEqual([
      { type: 'EntryCheckpoint', reason: 'progress', note: 'keep going', createdAt: 5 },
      { type: 'EntryCheckpoint', reason: 'milestone', note: null, createdAt: 6 },
      { type: 'RunSummary', iterations: 0, toolCalls: 0, elapsedMs: 0 },
    ])
  })

  it('drops malformed or unknown entries', () => {
    const entries = [
      { id: '1', session_id: 's', created_at: 1, type: 'message', data: { message: {} } },
      { id: '2', session_id: 's', created_at: 2, type: 'weird_type', data: { x: 1 } },
      { id: '3', session_id: 's', created_at: 3, type: 'message', data: { message: { message: { role: 'system', content: 'ignored' } } } },
    ] as SessionEntry[]

    expect(sessionEntriesToConversation(entries)).toEqual([])
  })
})
