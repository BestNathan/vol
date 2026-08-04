// frontend/tests/unit/log-viewer.test.ts
import { describe, it, expect } from 'vitest'
import { entryColor, shortRunId, isLogViewerCacheState } from '@/components/panels/LogViewer'
import type { LogLine, LogRunSummary } from '@/types'

describe('entryColor', () => {
  it('colors agent lifecycle events green', () => {
    expect(entryColor('AgentStart')).toBe('#40c040')
    expect(entryColor('AgentComplete')).toBe('#40c040')
  })

  it('colors tool call begin/complete yellow', () => {
    expect(entryColor('ToolCallBegin')).toBe('#c0c040')
    expect(entryColor('ToolCallComplete')).toBe('#c0c040')
  })

  it('colors errors and aborts red', () => {
    expect(entryColor('ToolCallError')).toBe('#c04040')
    expect(entryColor('AgentAborted')).toBe('#c04040')
  })

  it('defaults unknown event types to grey', () => {
    expect(entryColor('AgentThinking')).toBe('#e0e0e0')
    expect(entryColor('')).toBe('#e0e0e0')
  })
})

describe('shortRunId', () => {
  it('leaves short ids untouched', () => {
    expect(shortRunId('abc123')).toBe('abc123')
    expect(shortRunId('123456789012')).toBe('123456789012')
  })

  it('truncates long ids to 9 chars plus ellipsis', () => {
    expect(shortRunId('1234567890123')).toBe('123456789...')
    expect(shortRunId('run-abcdefghijklmnop')).toBe('run-abcde...')
  })
})

describe('isLogViewerCacheState', () => {
  const run: LogRunSummary = { run_id: 'r1', event_count: 2, last_event: 'AgentComplete', last_event_time: '12:00' }
  const line: LogLine = { timestamp: '12:00', event_type: 'AgentStart', summary: 'begin' }

  it('accepts a complete viewer state object', () => {
    const state = { run_logs: [run], entries: [line], selected_run: null, loading: false, error: null }
    expect(isLogViewerCacheState(state)).toBe(true)
  })

  it('rejects values missing the run_logs/entries arrays', () => {
    expect(isLogViewerCacheState(null)).toBe(false)
    expect(isLogViewerCacheState({ run_logs: [], loading: false })).toBe(false)
    expect(isLogViewerCacheState([run])).toBe(false)
  })
})
