// frontend/src/lib/reconnect.ts
const MAX_ATTEMPTS = 10
const MIN_DELAY = 3
const MAX_DELAY = 30

function delay(seconds: number): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, seconds * 1000))
}

/** Minimal surface of a JSON-RPC client required for reconnection. */
export interface ReconnectTarget {
  reconnect(): void
  /** Connection state; a boolean or a predicate for lazy evaluation. */
  _connected?: boolean | (() => boolean)
}

function isConnected(target: ReconnectTarget): boolean {
  const state = target._connected
  return typeof state === 'function' ? state() : Boolean(state)
}

export async function attemptReconnect(
  target: ReconnectTarget,
  onAttempt: (attempt: number, delaySecs: number) => void = () => {},
): Promise<boolean> {
  // Already connected — nothing to do
  if (isConnected(target)) return true

  for (let attempt = 1; attempt <= MAX_ATTEMPTS; attempt++) {
    const delaySecs = Math.min(MIN_DELAY * Math.pow(2, attempt - 1), MAX_DELAY)
    onAttempt(attempt, delaySecs)

    await delay(delaySecs)

    if (isConnected(target)) return true

    target.reconnect()

    // Wait briefly for connection to establish
    await delay(1)
    if (isConnected(target)) return true
  }

  return false
}
