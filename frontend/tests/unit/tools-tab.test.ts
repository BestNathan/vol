// frontend/tests/unit/tools-tab.test.ts
import { describe, it, expect } from 'vitest'
import { formatToolCallResult, filterToolList } from '@/components/panels/ToolsTab'

function tool(name: string, description: string) {
  return { name, description, parameters: undefined }
}

describe('formatToolCallResult', () => {
  it('extracts the content string from the tool.call result envelope', () => {
    const result = {
      tool_name: 'bash',
      result: { success: true, content: 'hello world', error: null, data: null },
    }
    expect(formatToolCallResult(result)).toBe('hello world')
  })

  it('falls back to pretty-printed JSON when content is absent', () => {
    const result = {
      tool_name: 'bash',
      result: { success: false, content: null, error: 'boom', data: { code: 1 } },
    }
    const text = formatToolCallResult(result)
    expect(text).toContain('"tool_name": "bash"')
    expect(text).toContain('"error": "boom"')
    expect(text).toContain('\n')
  })

  it('handles non-object results without throwing', () => {
    expect(formatToolCallResult(null)).toBe('null')
    expect(formatToolCallResult('raw')).toBe('"raw"')
    expect(formatToolCallResult(42)).toBe('42')
  })
})

describe('filterToolList', () => {
  const tools = [
    tool('bash', 'Execute shell commands'),
    tool('read', 'Read a file from disk'),
    tool('grep', 'Search file contents with regex'),
  ]

  it('returns all tools when search is empty', () => {
    expect(filterToolList(tools, '')).toHaveLength(3)
  })

  it('matches by tool name (case-insensitive)', () => {
    expect(filterToolList(tools, 'BASH')).toHaveLength(1)
    expect(filterToolList(tools, 'BASH')[0].name).toBe('bash')
  })

  it('matches by description (case-insensitive)', () => {
    expect(filterToolList(tools, 'regex')).toHaveLength(1)
    expect(filterToolList(tools, 'regex')[0].name).toBe('grep')
  })

  it('returns empty array when nothing matches', () => {
    expect(filterToolList(tools, 'nonexistent')).toHaveLength(0)
  })
})
