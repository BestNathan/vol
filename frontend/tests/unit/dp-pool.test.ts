// frontend/tests/unit/dp-pool.test.ts
import { describe, it, expect, vi, beforeEach } from 'vitest'

// Mock JsonRpcClient
vi.mock('@/lib/jsonrpc-client', () => ({
  JsonRpcClient: vi.fn().mockImplementation((url: string) => ({
    url,
    call: vi.fn().mockResolvedValue(null),
    onStateChange: vi.fn(),
    onEvent: vi.fn(() => () => {}),
    reconnect: vi.fn(),
  }))
}))

import { DpConnectionPool } from '@/lib/dp-pool'
import { JsonRpcClient } from '@/lib/jsonrpc-client'

describe('DpConnectionPool', () => {
  // Vitest does not clear mock call history between tests; per-test
  // call-count assertions need a fresh history.
  beforeEach(() => { vi.clearAllMocks() })

  it('getOrCreate lazily creates connection for new node', () => {
    const pool = new DpConnectionPool()
    const client = pool.getOrCreate('node1', 'ws://n1/ws')

    expect(JsonRpcClient).toHaveBeenCalledWith('ws://n1/ws')
    expect(client).toBeDefined()
  })

  it('getOrCreate returns existing connection for same node', () => {
    const pool = new DpConnectionPool()
    const c1 = pool.getOrCreate('node1', 'ws://n1/ws')
    const c2 = pool.getOrCreate('node1', 'ws://n1/ws')

    expect(c1).toBe(c2)
    expect(JsonRpcClient).toHaveBeenCalledTimes(1)
  })

  it('get returns undefined for unknown node', () => {
    const pool = new DpConnectionPool()
    expect(pool.get('unknown')).toBeUndefined()
  })

  it('connections iterates all entries', () => {
    const pool = new DpConnectionPool()
    pool.getOrCreate('n1', 'ws://n1/ws')
    pool.getOrCreate('n2', 'ws://n2/ws')

    const entries = pool.connections()
    expect(entries.length).toBe(2)
  })
})
