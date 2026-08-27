// frontend/src/App.tsx
import { useEffect, useRef, useCallback } from 'react'
import { useAtomValue, useSetAtom, getDefaultStore } from 'jotai'
import { Provider } from 'jotai'
import { StatusBar } from '@/components/layout/StatusBar'
import { TabBar } from '@/components/layout/TabBar'
import { TabContent } from '@/components/layout/TabContent'
import { ApprovalDialog } from '@/components/dialogs/ApprovalDialog'
import { DebugPanel } from '@/components/dialogs/DebugPanel'
import { Tabs } from '@/components/ui/tabs'
import { FileTree } from '@/components/panels/FileTree'
import { NodesPanel } from '@/components/panels/NodesPanel'
import { JsonRpcClient } from '@/lib/jsonrpc-client'
import { deriveWsUrl } from '@/lib/ws-url'
import { initClients } from '@/lib/panel-client'
import { dpPool } from '@/lib/dp-pool'
import { attemptReconnect } from '@/lib/reconnect'
import { agentEventToUiEvent, handleUiEvent } from '@/lib/event-handlers'
import {
  connectionStateAtom,
  serverModeAtom,
  wsUrlAtom,
  wsConnectedAtom,
  wsLastErrorAtom,
  isRunningAtom,
  runningAgentsAtom,
  runMapAtom,
  pendingSubmitAgentAtom,
} from '@/stores/connection'
import { approvalPendingAtom } from '@/stores/dialogs'
import { activeNodeIdAtom, activeTabAtom, LOCAL_NODE_ID, viewingNodeDetailAtom } from '@/stores/ui'
import { debugPanelAtom } from '@/stores/dialogs'
import type { ConnectedInfo } from '@/types'

function AppInner() {
  const setConnectionState = useSetAtom(connectionStateAtom)
  const setServerMode = useSetAtom(serverModeAtom)
  const setWsUrl = useSetAtom(wsUrlAtom)
  const setWsConnected = useSetAtom(wsConnectedAtom)
  const setWsLastError = useSetAtom(wsLastErrorAtom)
  const setActiveNodeId = useSetAtom(activeNodeIdAtom)
  const viewingNodeDetail = useAtomValue(viewingNodeDetailAtom)
  const activeTab = useAtomValue(activeTabAtom)
  const setDebugPanel = useSetAtom(debugPanelAtom)
  const clientRef = useRef<JsonRpcClient | null>(null)
  const debugStartRef = useRef<number | null>(null)
  const reconnectAbortRef = useRef<AbortController | null>(null)

  const startReconnect = useCallback(
    (client: JsonRpcClient) => {
      // Cancel any previous reconnect loop
      reconnectAbortRef.current?.abort()
      const ac = new AbortController()
      reconnectAbortRef.current = ac

      attemptReconnect(client, (_attempt, delaySecs) => {
        if (ac.signal.aborted) return
        setWsConnected(false)
        setWsLastError(`Reconnecting (${delaySecs}s)`)
      }).then((ok) => {
        if (ac.signal.aborted) return
        if (!ok) {
          setWsLastError('Connection lost. Please refresh.')
        }
      })
    },
    [setWsConnected, setWsLastError],
  )

  useEffect(() => {
    const url = deriveWsUrl()
    setWsUrl(url)
    const client = new JsonRpcClient(url)
    clientRef.current = client

    // Initialise the connection routing layer: one CP connection for
    // control-plane ops, plus the DP pool for per-node agent connections.
    initClients(client)

    // Spawn event stream consumer — handle events from the main connection.
    let running = true
    const handleAgentEvent = (agentEvent: Parameters<Parameters<typeof client.onEvent>[0]>[0]) => {
      if (!running) return
      const rawEvent = agentEvent.event
      const entries = Object.entries(rawEvent)
      if (entries.length === 0) return
      const [variant, data] = entries[0]
      const uiEvent = agentEventToUiEvent(
        variant,
        data as Record<string, unknown>,
        agentEvent.run_id,
      )
      if (uiEvent) {
        handleUiEvent(uiEvent, agentEvent.run_id)
      }
    }
    client.onEvent(handleAgentEvent)

    // Register the same handler on DP pool connections — in CP mode agent
    // events come through per-node DP connections, not the main connection.
    dpPool.setEventHandler(handleAgentEvent)

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

    client.onStateChange((state) => {
      setConnectionState(state)
      if (state === 'connected') {
        setWsConnected(true)
        setWsLastError(null)
        // Cancel any running reconnect loop
        reconnectAbortRef.current?.abort()
        client
          .call<ConnectedInfo>('system.connected')
          .then((info) => {
            setServerMode(info.server_type)
            if (info.server_type === 'DataPlane') {
              setActiveNodeId(LOCAL_NODE_ID)
            }
          })
          .catch(() => {})
      } else if (state === 'disconnected') {
        setWsConnected(false)
        setWsLastError('Disconnected')
        // Reset transient run state so the UI doesn't stay locked in "Running"
        // when the backend connection drops mid-run. On reconnect the user can
        // re-select the agent, which re-queries agent.status.
        const store = getDefaultStore()
        store.set(isRunningAtom, false)
        store.set(runningAgentsAtom, new Set())
        store.set(runMapAtom, new Map())
        store.set(pendingSubmitAgentAtom, null)
        store.set(approvalPendingAtom, false)
        startReconnect(client)
      }
    })

    return () => {
      running = false
      reconnectAbortRef.current?.abort()
      client.close()
    }
  }, [
    setConnectionState,
    setServerMode,
    setWsUrl,
    setWsConnected,
    setWsLastError,
    setDebugPanel,
    startReconnect,
  ])

  return (
    <div className="relative h-[100dvh] w-[100vw] font-[system-ui] text-[14px] text-[#e0e0e0] bg-[#1a1a2e]">
      <div className="flex flex-col h-full w-full overflow-hidden">
        <StatusBar />
        {viewingNodeDetail ? (
          <NodesPanel />
        ) : (
          <div className="flex flex-1 overflow-hidden relative">
            <FileTree />
            <Tabs value={activeTab} className="flex-1 min-h-0 overflow-hidden flex flex-col">
              <TabBar />
              <TabContent />
            </Tabs>
          </div>
        )}
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
