// frontend/src/App.tsx
import { useEffect, useRef } from 'react'
import { useSetAtom } from 'jotai'
import { Provider } from 'jotai'
import { StatusBar } from '@/components/layout/StatusBar'
import { TabBar } from '@/components/layout/TabBar'
import { TabContent } from '@/components/layout/TabContent'
import { FileTree } from '@/components/panels/FileTree'
import { JsonRpcClient } from '@/lib/jsonrpc-client'
import { deriveWsUrl } from '@/lib/ws-url'
import { connectionStateAtom, serverModeAtom, wsUrlAtom } from '@/stores/connection'

function AppInner() {
  const setConnectionState = useSetAtom(connectionStateAtom)
  const setServerMode = useSetAtom(serverModeAtom)
  const setWsUrl = useSetAtom(wsUrlAtom)
  const clientRef = useRef<JsonRpcClient | null>(null)

  useEffect(() => {
    const url = deriveWsUrl()
    setWsUrl(url)
    const client = new JsonRpcClient(url)
    clientRef.current = client

    client.onStateChange((state) => {
      setConnectionState(state)
      if (state === 'connected') {
        client.call<{ server_type: string }>('system.connected').then(info => {
          setServerMode(info.server_type as any)
        }).catch(() => {})
      }
    })

    return () => { /* cleanup handled by browser */ }
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
    </div>
  )
}

export function App() {
  return (
    <Provider>
      <AppInner />
    </Provider>
  )
}
