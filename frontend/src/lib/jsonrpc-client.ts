// frontend/src/lib/jsonrpc-client.ts
import type { ConnectionState } from '@/types'
import type { AgentEvent } from './protocol'

type ResponseCallback = (result: unknown) => void
type ErrorCallback = (error: { code: number; message: string }) => void

// Debug capture hook (DebugPanel WS inspector): notified for every outbound
// JSON-RPC request and every inbound WS message, with the extracted method
// name and the raw payload string.
export interface DebugCapture {
  direction: 'in' | 'out'
  method: string
  payload: string
}

interface PendingCall {
  resolve: ResponseCallback
  reject: ErrorCallback
}

export class JsonRpcClient {
  private ws: WebSocket | null = null
  private url: string
  private nextId = 1
  private pending = new Map<number, PendingCall>()
  private stateChangeCallback: ((state: ConnectionState) => void) | null = null
  private autoSubscribe: boolean
  private sendQueue: string[] = []
  private state: ConnectionState = 'connecting'
  private debugCapture: ((capture: DebugCapture) => void) | null = null

  // Event stream: push-based via callbacks stored by consumers
  private eventListeners: Array<(event: AgentEvent) => void> = []
  // Notifications received before any consumer subscribed are buffered here
  private eventBuffer: AgentEvent[] = []

  constructor(url: string, opts?: { autoSubscribe?: boolean }) {
    this.url = url
    this.autoSubscribe = opts?.autoSubscribe ?? true
    this.connect()
  }

  isConnected(): boolean {
    return this.state === 'connected'
  }

  close(): void {
    if (this.ws) {
      this.ws.onclose = null
      this.ws.onerror = null
      this.ws.onmessage = null
      this.ws.close()
      this.ws = null
    }
    this.state = 'disconnected'
    // Fail all pending calls
    for (const [, { reject }] of this.pending) {
      reject({ code: -1, message: 'WebSocket closed' })
    }
    this.pending.clear()
    this.sendQueue = []
  }

  private connect(): void {
    this.state = 'connecting'
    this.stateChangeCallback?.('connecting')
    const ws = new WebSocket(this.url)
    this.ws = ws

    ws.onopen = () => {
      this.state = 'connected'
      this.stateChangeCallback?.('connected')
      // Flush send queue
      for (const msg of this.sendQueue) {
        ws.send(msg)
      }
      this.sendQueue = []
      // Auto-subscribe to agent events
      if (this.autoSubscribe) {
        this.call('agent.subscribe').catch(() => {})
      }
    }

    ws.onmessage = (e: MessageEvent) => {
      const msg = JSON.parse(e.data as string)
      if (msg.id != null && this.pending.has(msg.id)) {
        const { resolve, reject } = this.pending.get(msg.id)!
        this.pending.delete(msg.id)
        if (msg.error) {
          reject(msg.error)
        } else {
          resolve(msg.result)
        }
      } else if (msg.method === 'agent.event' && msg.params) {
        const event: AgentEvent = msg.params
        if (this.eventListeners.length === 0) {
          this.eventBuffer.push(event)
        }
        for (const listener of this.eventListeners) {
          listener(event)
        }
      }
      // Capture for debug AFTER processing — never blocks message handling
      this.debugCapture?.({
        direction: 'in',
        method: typeof msg.method === 'string' ? msg.method : '<response>',
        payload: e.data as string,
      })
    }

    ws.onclose = () => {
      this.state = 'disconnected'
      this.stateChangeCallback?.('disconnected')
      // Fail all pending calls
      for (const [, { reject }] of this.pending) {
        reject({ code: -1, message: 'WebSocket disconnected' })
      }
      this.pending.clear()
    }

    ws.onerror = () => {
      /* onclose will fire after this */
    }
  }

  call<T>(method: string, params?: unknown): Promise<T> {
    return new Promise((resolve, reject) => {
      const id = this.nextId++
      const request = { jsonrpc: '2.0', method, params: params ?? {}, id }
      const message = JSON.stringify(request)

      this.pending.set(id, { resolve: resolve as ResponseCallback, reject })

      // Capture for debug (DebugPanel WS inspector) — includes internal calls
      // such as agent.subscribe and system.connected
      this.debugCapture?.({ direction: 'out', method, payload: message })

      if (this.ws?.readyState === WebSocket.OPEN) {
        this.ws.send(message)
      } else {
        this.sendQueue.push(message)
      }
    })
  }

  /** Attach the DebugPanel WS capture hook (null detaches). */
  setDebugCapture(cb: ((capture: DebugCapture) => void) | null): void {
    this.debugCapture = cb
  }

  onStateChange(cb: (state: ConnectionState) => void): void {
    this.stateChangeCallback = cb
    // Deliver the current state immediately so late registrants see it
    cb(this.state)
  }

  reconnect(): void {
    if (this.ws) {
      this.ws.onclose = null // prevent disconnect callback firing
      this.ws.close()
    }
    this.connect()
  }

  onEvent(listener: (event: AgentEvent) => void): () => void {
    this.eventListeners.push(listener)
    return () => {
      this.eventListeners = this.eventListeners.filter((l) => l !== listener)
    }
  }

  eventStream(): AsyncIterable<AgentEvent> {
    // Pre-seed with events buffered before the stream was opened
    const queue: AgentEvent[] = this.eventBuffer.splice(0)
    const waiters: Array<(event: AgentEvent) => void> = []
    const unsubscribe = this.onEvent((event) => {
      const waiter = waiters.shift()
      if (waiter) {
        waiter(event)
      } else {
        queue.push(event)
      }
    })
    return {
      [Symbol.asyncIterator]() {
        return {
          next: (): Promise<IteratorResult<AgentEvent>> => {
            const event = queue.shift()
            if (event) {
              return Promise.resolve({ value: event, done: false })
            }
            return new Promise((resolve) => {
              waiters.push((event) => resolve({ value: event, done: false }))
            })
          },
          return: (): Promise<IteratorResult<AgentEvent>> => {
            unsubscribe()
            return Promise.resolve({ done: true, value: undefined })
          },
        }
      },
    }
  }
}
