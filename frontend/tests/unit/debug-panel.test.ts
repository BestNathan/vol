// frontend/tests/unit/debug-panel.test.ts
import { describe, it, expect, vi } from 'vitest'
import { formatElapsed, formatJsonPretty } from '@/components/dialogs/DebugPanel'

describe('formatElapsed', () => {
  it('formats as HH:MM:SS.mmm', () => {
    expect(formatElapsed(0)).toBe('00:00:00.000')
    expect(formatElapsed(61_500)).toBe('00:01:01.500')
    expect(formatElapsed(3_661_123)).toBe('01:01:01.123')
    expect(formatElapsed(3_725_000)).toBe('01:02:05.000')
  })
})

describe('formatJsonPretty', () => {
  it('pretty-prints valid JSON', () => {
    expect(formatJsonPretty('{"a":1,"b":[1,2]}')).toBe(
      '{\n  "a": 1,\n  "b": [\n    1,\n    2\n  ]\n}',
    )
  })

  it('returns raw string for invalid JSON', () => {
    expect(formatJsonPretty('not json')).toBe('not json')
    expect(formatJsonPretty('')).toBe('')
  })
})

describe('JsonRpcClient debug capture', () => {
  class MockWebSocket {
    onopen: (() => void) | null = null
    onmessage: ((e: { data: string }) => void) | null = null
    onclose: ((e: { code: number }) => void) | null = null
    onerror: (() => void) | null = null
    readyState = 0 // CONNECTING
    sent: string[] = []
    static readonly CONNECTING = 0
    static readonly OPEN = 1
    static readonly CLOSING = 2
    static readonly CLOSED = 3

    constructor(public url: string) {
      setTimeout(() => { this.readyState = 1; this.onopen?.() }, 0)
    }
    send(data: string) { this.sent.push(data) }
    close() { this.readyState = 3; this.onclose?.({ code: 1000 }) }
    receive(data: object) { this.onmessage?.({ data: JSON.stringify(data) }) }
  }

  vi.stubGlobal('WebSocket', MockWebSocket)

  it('captures outbound calls and inbound messages with method extraction', async () => {
    const { JsonRpcClient } = await import('@/lib/jsonrpc-client')
    const client = new JsonRpcClient('ws://test/ws', { autoSubscribe: false })
    await new Promise((r) => setTimeout(r, 10))

    const captures: Array<{ direction: string; method: string; payload: string }> = []
    client.setDebugCapture((c) => captures.push(c))

    client.call('ping', { n: 1 })
    const ws = (client as unknown as { ws: MockWebSocket }).ws
    ws.receive({ jsonrpc: '2.0', id: 1, result: 'pong' })
    ws.receive({ jsonrpc: '2.0', method: 'agent.event', params: { event: {} } })

    expect(captures.map((c) => [c.direction, c.method])).toEqual([
      ['out', 'ping'],
      ['in', '<response>'],
      ['in', 'agent.event'],
    ])
    // Raw payloads preserved (outbound serialized request / inbound message)
    expect(JSON.parse(captures[0].payload)).toMatchObject({ method: 'ping', params: { n: 1 } })
    expect(JSON.parse(captures[1].payload)).toMatchObject({ result: 'pong' })
    expect(JSON.parse(captures[2].payload)).toMatchObject({ method: 'agent.event' })
  })
})
