// frontend/src/components/inputs/CapabilityBar.tsx
// Capability summary bar — sits between the conversation and the input area.
// Shows "🛠 N tools · N skills · N MCPs" from capOverlayAtom.effective_*;
// the ✎ button opens the right-side CapabilityDrawer; the Attach button next
// to it picks images into the shared imageAttachmentsAtom. Fetches
// capabilities on agent change so the summary counts stay fresh. Port of
// capability_bar.rs.
import { useEffect, useRef } from 'react'
import { useAtom, useAtomValue, useSetAtom } from 'jotai'
import { PaperclipIcon } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { getPanelClient } from '@/lib/panel-client'
import { cn } from '@/lib/utils'
import { useImageAttachments } from '@/hooks/useImageAttachments'
import { selectedAgentIdAtom } from '@/stores/agents'
import { sessionIdAtom, isRunningAtom } from '@/stores/connection'
import { approvalPendingAtom } from '@/stores/dialogs'
import { capOverlayAtom, drawerOpenAtom } from '@/stores/capability'
import type { GetCapabilitiesResult } from '@/lib/protocol'

export function CapabilityBar() {
  const selectedAgentId = useAtomValue(selectedAgentIdAtom)
  const sessionId = useAtomValue(sessionIdAtom)
  const isRunning = useAtomValue(isRunningAtom)
  const approvalPending = useAtomValue(approvalPendingAtom)
  const [overlay, setOverlay] = useAtom(capOverlayAtom)
  const setDrawerOpen = useSetAtom(drawerOpenAtom)
  const { addFiles } = useImageAttachments()
  const fileInputRef = useRef<HTMLInputElement>(null)

  // Load capabilities when the selected agent (or session) changes. The
  // drawer re-fetches on open; this fetch only fills effective_* so the
  // summary counts render without opening the drawer.
  useEffect(() => {
    if (!selectedAgentId) {
      // No agent selected — clear the loading flag so the bar does not stay
      // stuck on "Loading capabilities...".
      setOverlay((o) => ({ ...o, loading: false }))
      return
    }
    setOverlay((o) => ({ ...o, loading: true }))
    let stale = false
    getPanelClient()
      .call<GetCapabilitiesResult>('agent.get_capabilities', {
        agent_id: selectedAgentId,
        session_id: sessionId,
      })
      .then((res) => {
        if (stale) return
        setOverlay((o) => ({
          ...o,
          effective_tools: res.effective_tools,
          effective_skills: res.effective_skills,
          effective_mcp_servers: res.effective_mcp_servers,
          loading: false,
        }))
      })
      .catch((err) => {
        if (stale) return
        console.error('Failed to load capabilities:', err)
        setOverlay((o) => ({ ...o, loading: false }))
      })
    return () => {
      stale = true
    }
  }, [selectedAgentId, sessionId, setOverlay])

  const { effective_tools, effective_skills, effective_mcp_servers, loading } = overlay

  return (
    <div className="flex items-center gap-2 px-3 py-1.5 border-t border-[#2a2a44] bg-[#181825] text-[12px] flex-shrink-0">
      {loading ? (
        <span className="text-muted-foreground/70">Loading capabilities...</span>
      ) : (
        <>
          <span className="text-muted-foreground">
            🛠 {effective_tools.length} tools · {effective_skills.length} skills ·{' '}
            {effective_mcp_servers.length} MCPs
          </span>
          <button
            type="button"
            disabled={!selectedAgentId}
            onClick={() => setDrawerOpen(true)}
            aria-label="Edit capabilities"
            className={cn(
              'ml-1 px-1.5 py-0.5 text-[11px] bg-secondary rounded',
              selectedAgentId
                ? 'text-foreground/70 hover:bg-border hover:text-foreground/80 cursor-pointer'
                : 'text-muted-foreground/60 cursor-not-allowed',
            )}
          >
            ✎
          </button>
        </>
      )}
      <input
        ref={fileInputRef}
        type="file"
        accept="image/*"
        multiple
        className="hidden"
        onChange={(e) => {
          addFiles(Array.from(e.target.files ?? []))
          e.target.value = ''
        }}
      />
      <Button
        variant="ghost"
        size="sm"
        className="cursor-pointer text-muted-foreground/60 hover:text-yellow-400/70 text-[11px]"
        onClick={() => fileInputRef.current?.click()}
        disabled={isRunning || approvalPending}
        aria-label="Attach images"
      >
        <PaperclipIcon data-icon="inline-start" />
        Attach
      </Button>
    </div>
  )
}
