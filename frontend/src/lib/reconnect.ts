// frontend/src/lib/reconnect.ts
const MAX_ATTEMPTS = 10
const MIN_DELAY = 3
const MAX_DELAY = 30

function delay(seconds: number): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, seconds * 1000))
}

export async function attemptReconnect(
  client: { reconnect(): void; isConnected(): boolean },
  onAttempt: (attempt: number, delaySecs: number) => void = () => {},
): Promise<boolean> {
  // Already connected — nothing to do
  if (client.isConnected()) return true

  for (let attempt = 1; attempt <= MAX_ATTEMPTS; attempt++) {
    const delaySecs = Math.min(MIN_DELAY * Math.pow(2, attempt - 1), MAX_DELAY)
    onAttempt(attempt, delaySecs)

    await delay(delaySecs)

    if (client.isConnected()) return true

    client.reconnect()

    // Wait briefly for connection to establish
    await delay(1)
    if (client.isConnected()) return true
  }

  return false
}
