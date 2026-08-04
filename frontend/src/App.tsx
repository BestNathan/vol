// frontend/src/App.tsx
import { useEffect, useRef, useCallback } from 'react'
import { useSetAtom, getDefaultStore } from 'jotai'
import { Provider } from 'jotai'
import { StatusBar } from '@/components/layout/StatusBar'
import { TabBar } from '@/components/layout/TabBar'
import { TabContent } from '@/components/layout/TabContent'
import { ApprovalDialog } from '@/components/dialogs/ApprovalDialog'
import { DebugPanel } from '@/components/dialogs/DebugPanel'
import { FileTree } from '@/components/panels/FileTree'
import { JsonRpcClient } from '@/lib/jsonrpc-client'
import { deriveWsUrl } from '@/lib/ws-url'
import { initClients } from '@/lib/panel-client'
import { attemptReconnect } from '@/lib/reconnect'
import { agentEventToUiEvent, handleUiEvent } from '@/lib/event-handlers'
import {
  connectionStateAtom, serverModeAtom, wsUrlAtom,
  wsConnectedAtom, wsLastErrorAtom,
} from '@/stores/connection'
import { debugPanelAtom } from '@/stores/dialogs'

function AppInner() {
  const setConnectionState = useSetAtom(connectionStateAtom)
  const setServerMode = useSetAtom(serverModeAtom)
  const setWsUrl = useSetAtom(wsUrlAtom)
  const setWsConnected = useSetAtom(wsConnectedAtom)
  const setWsLastError = useSetAtom(wsLastErrorAtom)
  const setDebugPanel = useSetAtom(debugPanelAtom)
  const clientRef = useRef<JsonRpcClient | null>(null)
  const debugStartRef = useRef<number | null>(null)
  const reconnectAbortRef = useRef<AbortController | null>(null)

  const startReconnect = useCallback((client: JsonRpcClient) => {
    // Cancel any previous reconnect loop
    reconnectAbortRef.current?.abort()
    const ac = new AbortController()
    reconnectAbortRef.current = ac

    attemptReconnect(
      client,
      (_attempt, delaySecs) => {
        if (ac.signal.aborted) return
        setWsConnected(false)
        setWsLastError(`Reconnecting (${delaySecs}s)`)
      },
    ).then((ok) => {
      if (ac.signal.aborted) return
      if (!ok) {
        setWsLastError('Connection lost. Please refresh.')
      }
    })
  }, [setWsConnected, setWsLastError])

  useEffect(() => {
    const url = deriveWsUrl()
    setWsUrl(url)
    const client = new JsonRpcClient(url)
    clientRef.current = client

    // Initialise the connection routing layer: one CP connection for
    // control-plane ops, plus the DP pool for per-node agent connections.
    initClients(client)

    // WS message capture for the DebugPanel
    client.setDebugCapture(({ direction, method, payload }) => {
      const now = performance.now()
      let start = debugStartRef.current
      if (start === null) {
        start = now
        debugStartRef.current = now
      }
      setDebugPanel((prev) => ({
        ...prev,
        messages: [...prev.messages, { direction, method, payload, elapsedMs: now - start }],
      }))
    })

    // Spawn event stream consumer
    let running = true
    client.onEvent((agentEvent) => {
      if (!running) return
      const rawEvent = agentEvent.event
      const entries = Object.entries(rawEvent)
      if (entries.length === 0) return
      const [variant, data] = entries[0]
      const uiEvent = agentEventToUiEvent(variant, data as Record<string, unknown>, agentEvent.run_id)
      if (uiEvent) {
        handleUiEvent(uiEvent, agentEvent.run_id)
      }
    })

    client.onStateChange((state) => {
      setConnectionState(state)
      if (state === 'connected') {
        setWsConnected(true)
        setWsLastError(null)
        // Cancel any running reconnect loop
        reconnectAbortRef.current?.abort()
        client.call<{ server_type: string }>('system.connected').then(info => {
          setServerMode(info.server_type as any)
        }).catch(() => {})
      } else if (state === 'disconnected') {
        setWsConnected(false)
        setWsLastError('Disconnected')
        startReconnect(client)
      }
    })

    return () => {
      running = false
      reconnectAbortRef.current?.abort()
    }
  }, [setConnectionState, setServerMode, setWsUrl, setWsConnected, setWsLastError, setDebugPanel, startReconnect])

  return (
    <div className="relative h-[100dvh] w-[100vw] font-[system-ui] text-[14px] text-[#e0e0e0] bg-[#1a1a2e]">
      <div className="flex flex-col h-full w-full overflow-hidden">
        <StatusBar />
        <div className="flex flex-1 overflow-hidden relative">
          <FileTree />
          <div className="min-w-0 flex-1 flex flex-col overflow-hidden">
            <TabBar />
            <TabContent />
          </div>
        </div>
      </div>
      <ApprovalDialog />
      <DebugPanel />
    </div>
  )
}

export function App() {
  return (
    <Provider store={getDefaultStore()}>
      <AppInner />
    </Provider>
  )
}
