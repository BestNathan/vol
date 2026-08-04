// frontend/tests/unit/jsonrpc-client.test.ts
import { describe, it, expect, vi, beforeEach } from 'vitest'

// Mock WebSocket
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
  // helper to simulate receiving a message
  receive(data: object) { this.onmessage?.({ data: JSON.stringify(data) }) }
}

// Replace global WebSocket
vi.stubGlobal('WebSocket', MockWebSocket)

// We need to dynamic-import the module after the mock is in place
async function importClient() {
  return import('@/lib/jsonrpc-client')
}

describe('JsonRpcClient', () => {
  it('connects and invokes state change callback', async () => {
    const { JsonRpcClient } = await importClient()
    const states: string[] = []
    const client = new JsonRpcClient('ws://test/ws')
    client.onStateChange((s) => states.push(s))

    await new Promise(r => setTimeout(r, 10))
    expect(states).toContain('connecting')
    expect(states).toContain('connected')
  })

  it('sends JSON-RPC request and resolves with result', async () => {
    const { JsonRpcClient } = await importClient()
    const client = new JsonRpcClient('ws://test/ws', { autoSubscribe: false })
    await new Promise(r => setTimeout(r, 10))

    const resultPromise = client.call<{ name: string }>('agent.list', { node_id: 'n1' })
    // Simulate server response (id:1 because first call)
    const ws = (client as any).ws as MockWebSocket
    ws.receive({ jsonrpc: '2.0', id: 1, result: [{ name: 'test-agent' }] })

    const result = await resultPromise
    expect(result).toEqual([{ name: 'test-agent' }])
    expect(ws.sent.length).toBe(1)
    const sent = JSON.parse(ws.sent[0])
    expect(sent.method).toBe('agent.list')
    expect(sent.params).toEqual({ node_id: 'n1' })
    expect(sent.id).toBe(1)
    expect(sent.jsonrpc).toBe('2.0')
  })

  it('handles error responses', async () => {
    const { JsonRpcClient } = await importClient()
    const client = new JsonRpcClient('ws://test/ws', { autoSubscribe: false })
    await new Promise(r => setTimeout(r, 10))

    const resultPromise = client.call('agent.list', {})
    const ws = (client as any).ws as MockWebSocket
    ws.receive({ jsonrpc: '2.0', id: 1, error: { code: -1, message: 'Not found' } })

    await expect(resultPromise).rejects.toEqual({ code: -1, message: 'Not found' })
  })

  it('routes notifications to event stream', async () => {
    const { JsonRpcClient } = await importClient()
    const client = new JsonRpcClient('ws://test/ws', { autoSubscribe: false })
    await new Promise(r => setTimeout(r, 10))

    const ws = (client as any).ws as MockWebSocket
    ws.receive({ jsonrpc: '2.0', method: 'agent.event', params: { run_id: 'r1', event: { AgentStart: { input: 'hello' } } } })

    // Read from event stream
    const iterator = client.eventStream()[Symbol.asyncIterator]()
    const { value } = await iterator.next()
    expect(value).toEqual({ run_id: 'r1', event: { AgentStart: { input: 'hello' } } })
  })

  it('reconnect creates new WebSocket', async () => {
    const { JsonRpcClient } = await importClient()
    const client = new JsonRpcClient('ws://test/ws', { autoSubscribe: false })
    await new Promise(r => setTimeout(r, 10))
    const oldWs = (client as any).ws

    client.reconnect()
    await new Promise(r => setTimeout(r, 10))
    expect((client as any).ws).not.toBe(oldWs)
  })
})
