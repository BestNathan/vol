// frontend/src/lib/ws-url.ts
export function deriveWsUrl(): string {
  if (typeof window === 'undefined') return 'ws://localhost:3001/ws'
  const hostname = window.location.hostname
  if (hostname === 'localhost' || hostname === '127.0.0.1') {
    return 'ws://localhost:3001/ws'
  }
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
  return `${protocol}//${window.location.host}/ws`
}
