// frontend/tests/unit/session-conversion.test.ts
import { describe, it, expect } from 'vitest'
import { sessionEntriesToConversation } from '@/lib/session-conversion'
import { formatAge, truncateId } from '@/components/panels/SessionsPanel'
import type { SessionEntry } from '@/lib/protocol'

// Real wire shape (vol-session SessionEntry, externally-tagged data):
//   data.message.message.message = { role, content, thinking, tool_calls, name }
// (data.message → { message: SessionMessage }, SessionMessage.message → Message)
function messageEntry(role: string, fields: Record<string, unknown> = {}): SessionEntry {
  return {
    id: 'id',
    session_id: 's',
    created_at: 1,
    type: 'message',
    data: { message: { message: { message: { role, ...fields } } } },
  } as unknown as SessionEntry
}

function checkpointEntry(reason: string, note: string | null = null): SessionEntry {
  return {
    id: 'id',
    session_id: 's',
    created_at: 5,
    type: 'checkpoint',
    data: { checkpoint: { reason, note } },
  } as unknown as SessionEntry
}

function summaryEntry(): SessionEntry {
  return {
    id: 'id',
    session_id: 's',
    created_at: 7,
    type: 'summary',
    data: { summary: { summary: 'done' } },
  } as unknown as SessionEntry
}

describe('sessionEntriesToConversation', () => {
  it('converts user, assistant (thinking + tool_calls), and tool messages', () => {
    const entries = [
      messageEntry('user', { content: 'hello' }),
      messageEntry('assistant', {
        content: [{ type: 'text', text: 'hi there' }],
        thinking: 'hmm',
        tool_calls: [{ name: 'bash', arguments: '{"cmd":"ls"}' }],
      }),
      messageEntry('tool', { name: 'bash', content: 'file1 file2' }),
    ]

    expect(sessionEntriesToConversation(entries)).toEqual([
      { type: 'UserInput', text: 'hello' },
      { type: 'Thinking', content: 'hmm' },
      { type: 'ToolCall', toolName: 'bash', argPreview: 'ls', fullArguments: '{"cmd":"ls"}' },
      { type: 'AgentAnswer', text: 'hi there' },
      { type: 'ToolResult', toolName: 'bash', preview: 'file1 file2', fullResult: 'file1 file2', success: true },
    ])
  })

  it('joins multipart content into a single text block', () => {
    const entries = [
      messageEntry('assistant', {
        content: [{ type: 'text', text: 'alpha' }, { type: 'text', text: 'beta' }],
      }),
    ]

    expect(sessionEntriesToConversation(entries)).toEqual([
      { type: 'AgentAnswer', text: 'alpha\nbeta' },
    ])
  })

  it('handles tool_calls arguments given as an object', () => {
    const entries = [
      messageEntry('assistant', {
        content: '',
        tool_calls: [{ name: 'read', arguments: { path: '/a.txt' } }],
      }),
    ]

    expect(sessionEntriesToConversation(entries)).toEqual([
      {
        type: 'ToolCall',
        toolName: 'read',
        argPreview: '/a.txt',
        fullArguments: '{"path":"/a.txt"}',
      },
      { type: 'AgentAnswer', text: '' },
    ])
  })

  it('skips empty thinking and defaults missing tool name/arguments', () => {
    const entries = [
      messageEntry('assistant', { content: 'plain answer', thinking: '' }),
      messageEntry('tool', { content: 'result text' }),
    ]

    expect(sessionEntriesToConversation(entries)).toEqual([
      { type: 'AgentAnswer', text: 'plain answer' },
      { type: 'ToolResult', toolName: 'tool', preview: 'result text', fullResult: 'result text', success: true },
    ])
  })

  it('handles plain-string content and missing optional fields', () => {
    const entries = [
      messageEntry('assistant', { content: 'plain answer' }),
      messageEntry('user'),
    ]

    expect(sessionEntriesToConversation(entries)).toEqual([
      { type: 'AgentAnswer', text: 'plain answer' },
      { type: 'UserInput', text: '' },
    ])
  })

  it('converts checkpoint and summary entries', () => {
    const entries = [
      checkpointEntry('progress', 'keep going'),
      checkpointEntry('milestone'),
      summaryEntry(),
    ]

    expect(sessionEntriesToConversation(entries)).toEqual([
      { type: 'EntryCheckpoint', reason: 'progress', note: 'keep going', createdAt: 5 },
      { type: 'EntryCheckpoint', reason: 'milestone', note: null, createdAt: 5 },
      { type: 'RunSummary', iterations: 0, toolCalls: 0, elapsedMs: 0 },
    ])
  })

  it('defaults a checkpoint reason and ignores the summary payload text', () => {
    const entries = [
      { ...checkpointEntry('compression'), data: { checkpoint: { note: null } } },
      summaryEntry(),
    ]

    expect(sessionEntriesToConversation(entries)).toEqual([
      { type: 'EntryCheckpoint', reason: 'Checkpoint', note: null, createdAt: 5 },
      { type: 'RunSummary', iterations: 0, toolCalls: 0, elapsedMs: 0 },
    ])
  })

  it('drops malformed or unknown entries', () => {
    const entries = [
      // Missing the inner Message (data.message.message exists but no .message).
      {
        id: '1', session_id: 's', created_at: 1, type: 'message',
        data: { message: { message: {} } },
      } as unknown as SessionEntry,
      { id: '2', session_id: 's', created_at: 2, type: 'weird_type', data: { x: 1 } } as unknown as SessionEntry,
      messageEntry('system', { content: 'ignored' }),
      // Message without a role.
      { id: '3', session_id: 's', created_at: 3, type: 'message', data: { message: { message: { message: { content: 'no role' } } } } } as unknown as SessionEntry,
    ]

    expect(sessionEntriesToConversation(entries)).toEqual([])
  })

  it('parses authentic persisted wire entries (data.message.message.message)', () => {
    // Slices of a real entry-store JSONL file (see vol-session/src/entry.rs):
    // SessionEntryData is externally tagged, so data.message is
    // { message: SessionMessage } and SessionMessage.message is the Message.
    const entries = [
      {
        id: 'bf01a5c7-4be0-4866-8d55-db170eb308b0',
        session_id: 'f98d7668-d00f-4983-90c6-cf6194e373bd',
        created_at: 1777204001,
        parent_id: null,
        type: 'message',
        data: {
          message: {
            message: {
              id: 'bf01a5c7-4be0-4866-8d55-db170eb308b0',
              session_id: 'f98d7668-d00f-4983-90c6-cf6194e373bd',
              message: { role: 'user', content: '详细分析当前项目并生成一个文档上传到飞书' },
              parent_id: null,
              created_at: 1777204001,
              metadata: { run_id: 'b044c812791b4cdd85d484bd89860289' },
            },
          },
        },
      },
      {
        id: '0eb80406-94bf-4930-b698-8523798cb436',
        session_id: 'f98d7668-d00f-4983-90c6-cf6194e373bd',
        created_at: 1777204024,
        parent_id: 'bf01a5c7-4be0-4866-8d55-db170eb308b0',
        type: 'message',
        data: {
          message: {
            message: {
              id: '0eb80406-94bf-4930-b698-8523798cb436',
              session_id: 'f98d7668-d00f-4983-90c6-cf6194e373bd',
              message: {
                role: 'assistant',
                content: 'Calling tools to get information.',
                tool_calls: [
                  {
                    id: 'toolu_15b0acbf7de34d7082b99c23',
                    name: 'bash',
                    arguments: '{"command": "ls -la && echo ---FILES--- && find . -maxdepth 3 -type f | head -80"}',
                    type: 'function',
                  },
                ],
              },
              parent_id: 'bf01a5c7-4be0-4866-8d55-db170eb308b0',
              created_at: 1777204024,
              metadata: { run_id: 'b044c812791b4cdd85d484bd89860289' },
            },
          },
        },
      },
      {
        id: '0c1c1bb6-115f-4cd9-a156-a8f5eef428a7',
        session_id: 'f98d7668-d00f-4983-90c6-cf6194e373bd',
        created_at: 1777204028,
        parent_id: '0eb80406-94bf-4930-b698-8523798cb436',
        type: 'message',
        data: {
          message: {
            message: {
              id: '0c1c1bb6-115f-4cd9-a156-a8f5eef428a7',
              session_id: 'f98d7668-d00f-4983-90c6-cf6194e373bd',
              message: {
                role: 'tool',
                name: 'bash',
                content: 'stdout:\ntotal 276\ndrwxr-xr-x 17 root root 4096 Apr 26 11:33 .',
              },
              parent_id: '0eb80406-94bf-4930-b698-8523798cb436',
              created_at: 1777204028,
              metadata: { run_id: 'b044c812791b4cdd85d484bd89860289' },
            },
          },
        },
      },
    ] as unknown as SessionEntry[]

    expect(sessionEntriesToConversation(entries)).toEqual([
      { type: 'UserInput', text: '详细分析当前项目并生成一个文档上传到飞书', images: undefined },
      {
        type: 'ToolCall',
        toolName: 'bash',
        argPreview: 'ls -la && echo ---FILES--- && find . -maxdepth 3 -type f | head -80',
        fullArguments: '{"command": "ls -la && echo ---FILES--- && find . -maxdepth 3 -type f | head -80"}',
        toolCallId: 'toolu_15b0acbf7de34d7082b99c23',
      },
      { type: 'AgentAnswer', text: 'Calling tools to get information.' },
      {
        type: 'ToolResult',
        toolName: 'bash',
        preview: 'stdout:\ntotal 276\ndrwxr-xr-x 17 root root 4096 Apr 26 11:33 .',
        fullResult: 'stdout:\ntotal 276\ndrwxr-xr-x 17 root root 4096 Apr 26 11:33 .',
        success: true,
        toolCallId: undefined,
      },
    ])
  })
})

describe('session list helpers', () => {
  it('formats ages as s/m/h/d ago', () => {
    const now = Math.floor(Date.now() / 1000)
    expect(formatAge(now - 42)).toBe('42s ago')
    expect(formatAge(now - 60)).toBe('1m ago')
    expect(formatAge(now - 3600)).toBe('1h ago')
    expect(formatAge(now - 86_400)).toBe('1d ago')
    expect(formatAge(now + 100)).toBe('0s ago') // future timestamps clamp to 0
  })

  it('truncates long session ids to 12 chars', () => {
    expect(truncateId('1234567890ab')).toBe('1234567890ab')
    expect(truncateId('1234567890abcdef')).toBe('1234567890ab...')
  })
})

// Loose fixtures on purpose: the extraction only reads `content`, so the entry
// arrays are cast at the call site (SessionEntry.data is `unknown`).
function userMessageEntry(content: unknown): unknown {
  return {
    type: 'message',
    created_at: 1,
    data: { message: { message: { message: { role: 'user', content } } } },
  }
}

describe('sessionEntriesToConversation — image parts', () => {
  it('extracts image URLs from multipart user content', () => {
    const entries = [
      userMessageEntry([
        { type: 'text', text: 'look at this' },
        { type: 'image', image_url: { url: 'data:image/png;base64,QUJD' } },
      ]),
    ] as unknown as SessionEntry[]
    const conv = sessionEntriesToConversation(entries)
    expect(conv).toHaveLength(1)
    expect(conv[0].type).toBe('UserInput')
    if (conv[0].type === 'UserInput') {
      expect(conv[0].text).toBe('look at this')
      expect(conv[0].images).toEqual(['data:image/png;base64,QUJD'])
    }
  })

  it('image-only multipart yields empty text with images', () => {
    const entries = [
      userMessageEntry([{ type: 'image', image_url: { url: 'https://e.test/a.png' } }]),
    ] as unknown as SessionEntry[]
    const conv = sessionEntriesToConversation(entries)
    expect(conv[0].type).toBe('UserInput')
    if (conv[0].type === 'UserInput') {
      expect(conv[0].text).toBe('')
      expect(conv[0].images).toEqual(['https://e.test/a.png'])
    }
  })

  it('text-only content has no images field', () => {
    const entries = [userMessageEntry('plain text')] as unknown as SessionEntry[]
    const conv = sessionEntriesToConversation(entries)
    expect(conv[0].type).toBe('UserInput')
    if (conv[0].type === 'UserInput') {
      expect(conv[0].text).toBe('plain text')
      expect(conv[0].images).toBeUndefined()
    }
  })
})
