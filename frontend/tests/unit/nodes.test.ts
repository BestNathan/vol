// frontend/tests/unit/nodes.test.ts
import { describe, it, expect, vi, afterEach } from 'vitest'
import { isNodeSelectable } from '@/components/shared/NodesDropdown'
import { formatAge } from '@/components/panels/NodeDetailPanel'
import type { NodeListEntry } from '@/types'

function node(overrides: Partial<NodeListEntry> = {}): NodeListEntry {
  return {
    node_id: 'n1',
    name: 'node-1',
    version: '1.0.0',
    status: 'online',
    capability_revision: 0,
    load: { running: 0, queued: 0 },
    ...overrides,
  }
}

describe('isNodeSelectable', () => {
  it('selects online nodes with a ws_url', () => {
    expect(isNodeSelectable(node({ status: 'online', ws_url: 'ws://n1/ws' }))).toBe(true)
  })

  it('rejects offline nodes even with a ws_url', () => {
    expect(isNodeSelectable(node({ status: 'offline', ws_url: 'ws://n1/ws' }))).toBe(false)
  })

  it('rejects online nodes without a ws_url', () => {
    expect(isNodeSelectable(node({ status: 'online', ws_url: undefined }))).toBe(false)
  })
})

describe('formatAge', () => {
  // "now" = 100_000_000 ms epoch so hour/day deltas stay positive.
  afterEach(() => { vi.restoreAllMocks() })

  function now100m(): void {
    vi.spyOn(Date, 'now').mockReturnValue(100_000_000)
  }

  it('formats seconds ago', () => {
    now100m()
    expect(formatAge(100_000_000)).toBe('0s ago')
    expect(formatAge(99_995_000)).toBe('5s ago')
  })

  it('formats minutes ago', () => {
    now100m()
    expect(formatAge(99_940_000)).toBe('1m ago')
    expect(formatAge(99_700_000)).toBe('5m ago')
  })

  it('formats hours and days ago', () => {
    now100m()
    expect(formatAge(96_400_000)).toBe('1h ago')
    expect(formatAge(13_600_000)).toBe('1d ago')
  })

  it('never goes negative for future timestamps', () => {
    now100m()
    expect(formatAge(200_000_000)).toBe('0s ago')
  })
})
