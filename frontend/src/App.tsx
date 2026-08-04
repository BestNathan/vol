// frontend/src/App.tsx
import { useEffect, useRef } from 'react'
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
import { agentEventToUiEvent, handleUiEvent } from '@/lib/event-handlers'
import { connectionStateAtom, serverModeAtom, wsUrlAtom } from '@/stores/connection'
import { debugPanelAtom } from '@/stores/dialogs'

function AppInner() {
  const setConnectionState = useSetAtom(connectionStateAtom)
  const setServerMode = useSetAtom(serverModeAtom)
  const setWsUrl = useSetAtom(wsUrlAtom)
  const setDebugPanel = useSetAtom(debugPanelAtom)
  const clientRef = useRef<JsonRpcClient | null>(null)
  const debugStartRef = useRef<number | null>(null)

  useEffect(() => {
    const url = deriveWsUrl()
    setWsUrl(url)
    const client = new JsonRpcClient(url)
    clientRef.current = client

    // WS message capture for the DebugPanel: every outbound call and every
    // inbound message is recorded with elapsed time since the first capture.
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

    // After "clientRef.current = client"
    const clientForEvents = client

    // Spawn event stream consumer
    let running = true
    ;(async () => {
      clientForEvents.onEvent((agentEvent) => {
        if (!running) return
        const rawEvent = agentEvent.event
        // Server sends externally-tagged: {"VariantName": {fields}}
        const entries = Object.entries(rawEvent)
        if (entries.length === 0) return
        const [variant, data] = entries[0]
        const uiEvent = agentEventToUiEvent(variant, data as Record<string, unknown>, agentEvent.run_id)
        if (uiEvent) {
          handleUiEvent(uiEvent, agentEvent.run_id)
        }
      })
    })()

    client.onStateChange((state) => {
      setConnectionState(state)
      if (state === 'connected') {
        client.call<{ server_type: string }>('system.connected').then(info => {
          setServerMode(info.server_type as any)
        }).catch(() => {})
      }
    })

    return () => { running = false }
  }, [])

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
      {/* Global HITL overlay: approval_request events arrive on any tab. */}
      <ApprovalDialog />
      {/* Global debug overlay: WS message inspector toggled from the StatusBar. */}
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
