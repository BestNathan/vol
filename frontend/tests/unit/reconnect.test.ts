// frontend/tests/unit/reconnect.test.ts
import { describe, it, expect, vi, beforeEach } from 'vitest'

// We need a controllable client mock
function createMockClient(isConnected: () => boolean) {
  return {
    reconnect: vi.fn(),
    call: vi.fn().mockResolvedValue(null),
    onStateChange: vi.fn(),
    onEvent: vi.fn(() => () => {}),
    _connected: isConnected,
  }
}

import { attemptReconnect } from '@/lib/reconnect'

describe('attemptReconnect', () => {
  beforeEach(() => { vi.useFakeTimers() })

  it('resolves true immediately if client is already connected', async () => {
    const client = createMockClient(() => true)
    const onAttempt = vi.fn()

    const resultPromise = attemptReconnect(client as any, onAttempt)
    // Fast-forward past any timers
    await vi.runAllTimersAsync()

    const result = await resultPromise
    expect(result).toBe(true)
    expect(onAttempt).not.toHaveBeenCalled()
  })

  it('tries up to 10 attempts with exponential backoff then fails', async () => {
    const client = createMockClient(() => false)
    const onAttempt = vi.fn()

    const resultPromise = attemptReconnect(client as any, onAttempt)

    // The function checks connected state after each reconnect call
    // Fast-forward all 10 attempts
    for (let i = 0; i < 10; i++) {
      await vi.advanceTimersByTimeAsync(1000) // wait for delay
      await vi.runAllTimersAsync()
    }

    const result = await resultPromise
    expect(result).toBe(false)
    expect(onAttempt).toHaveBeenCalledTimes(10)
    // Verify delays: 3,6,12,24,30,30,30,30,30,30
    expect(onAttempt.mock.calls[0][1]).toBe(3)
    expect(onAttempt.mock.calls[3][1]).toBe(24)
    expect(onAttempt.mock.calls[4][1]).toBe(30)
    expect(onAttempt.mock.calls[9][1]).toBe(30)
  })
})
