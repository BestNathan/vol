// frontend/tests/unit/mcp-dialogs.test.ts
// Unit tests for the pure helpers of the MCP panel and its dialogs:
// McpToolDialog's call-result formatter, PromptViewer's prompt/content
// formatters, and McpPanel's server status color mapping.
import { describe, expect, it } from 'vitest'
import { formatMcpCallResult } from '@/components/dialogs/McpToolDialog'
import { formatPromptResult, textFromPromptContent } from '@/components/dialogs/PromptViewer'
import { serverStatusColor } from '@/components/panels/McpPanel'

describe('formatMcpCallResult', () => {
  it('passes through a string result verbatim', () => {
    expect(formatMcpCallResult({ tool_name: 'git', result: '{"done": true}' })).toBe('{"done": true}')
  })

  it('pretty-prints a non-string result', () => {
    const out = formatMcpCallResult({ tool_name: 'git', result: { ok: true, n: 3 } })
    expect(out).toContain('"ok": true')
    expect(out).toContain('"n": 3')
  })

  it('returns empty string for a null result', () => {
    expect(formatMcpCallResult({ tool_name: 'git', result: null })).toBe('')
  })

  it('survives a missing result field', () => {
    expect(formatMcpCallResult({ tool_name: 'git', result: undefined })).toBe('')
  })
})

describe('textFromPromptContent', () => {
  it('extracts text blocks from a JSON content array', () => {
    const raw = JSON.stringify([
      { type: 'text', text: 'hello' },
      { type: 'text', text: 'world' },
    ])
    expect(textFromPromptContent(raw)).toBe('hello\nworld')
  })

  it('keeps plain non-JSON text as-is', () => {
    expect(textFromPromptContent('plain text')).toBe('plain text')
  })

  it('extracts the text field from a JSON object', () => {
    expect(textFromPromptContent(JSON.stringify({ type: 'text', text: 'obj text' }))).toBe('obj text')
  })

  it('returns empty string for null/undefined', () => {
    expect(textFromPromptContent(null)).toBe('')
    expect(textFromPromptContent(undefined)).toBe('')
  })
})

describe('formatPromptResult', () => {
  const prompt = {
    description: 'Fix this bug',
    messages: [
      { role: 'User', content: JSON.stringify([{ type: 'text', text: 'Please fix' }]) },
      { role: 'Assistant', content: JSON.stringify([{ type: 'text', text: 'Here is the fix' }]) },
    ],
  }

  it('formats description and messages as markdown', () => {
    const out = formatPromptResult(prompt)
    expect(out).toContain('Fix this bug')
    expect(out).toContain('### User')
    expect(out).toContain('Please fix')
    expect(out).toContain('### Assistant')
    expect(out).toContain('Here is the fix')
  })

  it('passes through a plain string prompt', () => {
    expect(formatPromptResult('just text')).toBe('just text')
  })

  it('falls back to pretty JSON for an empty prompt object', () => {
    expect(formatPromptResult({})).toBe('{}')
  })

  it('handles null/undefined without throwing', () => {
    expect(formatPromptResult(null)).toBe('null')
    expect(formatPromptResult(undefined)).toBe('')
  })
})

describe('serverStatusColor', () => {
  it('maps connected to green, disconnected to gray', () => {
    expect(serverStatusColor('connected')).toBe('#40c040')
    expect(serverStatusColor('disconnected')).toBe('#888')
  })

  it('maps connecting to yellow and anything else to red', () => {
    expect(serverStatusColor('connecting')).toBe('#f0c040')
    expect(serverStatusColor('error: boom')).toBe('#c04040')
    expect(serverStatusColor('')).toBe('#c04040')
  })
})
