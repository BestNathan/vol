// frontend/tests/unit/tools-tab.test.ts
import { describe, it, expect } from 'vitest'
import { formatToolCallResult, statusBadge } from '@/components/panels/ToolsTab'

describe('statusBadge', () => {
  it('maps Success to the OK badge in green', () => {
    const badge = statusBadge('Success')
    expect(badge.label).toBe('OK')
    expect(badge.className).toContain('#40c040')
  })

  it('maps Error to the ERR badge in red', () => {
    const badge = statusBadge('Error')
    expect(badge.label).toBe('ERR')
    expect(badge.className).toContain('#c04040')
  })

  it('maps Skipped to the SKIP badge in yellow', () => {
    const badge = statusBadge('Skipped')
    expect(badge.label).toBe('SKIP')
    expect(badge.className).toContain('#f0c040')
  })

  it('maps Running to the "..." badge in grey', () => {
    const badge = statusBadge('Running')
    expect(badge.label).toBe('...')
    expect(badge.className).toContain('#888')
  })
})

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
    expect(text).toContain('\n') // pretty-printed, not single-line
  })

  it('handles non-object results without throwing', () => {
    expect(formatToolCallResult(null)).toBe('null')
    expect(formatToolCallResult('raw')).toBe('"raw"')
    expect(formatToolCallResult(42)).toBe('42')
  })
})
