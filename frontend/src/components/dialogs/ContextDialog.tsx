// frontend/src/components/dialogs/ContextDialog.tsx
// Contributor snapshot modal: role-colored message blocks for the selected
// context contributor. Atom-driven via contextDialogAtom — the ContextPanel
// opens it by fetching agent.context_snapshot and setting open +
// contributorName. Port of context_panel.rs's ContextDialog, built on the
// shadcn Dialog like McpToolDialog.
import { useAtom } from 'jotai'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { contextDialogAtom } from '@/stores/context'

/** Message role text color: system grey, user blue, assistant white, tool gold. */
export function roleColor(role: string): string {
  switch (role) {
    case 'system': return '#888'
    case 'user': return '#80a0ff'
    case 'assistant': return '#e0e0e0'
    case 'tool': return '#c0a040'
    default: return '#888'
  }
}

export function ContextDialog() {
  const [dialog, setDialog] = useAtom(contextDialogAtom)
  const { open, contributorName, messages, loading, error } = dialog

  const close = () => setDialog({ open: false, contributorName: '', messages: [], loading: false })

  return (
    <Dialog open={open} onOpenChange={(next) => { if (!next) close() }}>
      <DialogContent className="sm:max-w-[700px]">
        <DialogHeader>
          <DialogTitle className="truncate">{contributorName}</DialogTitle>
        </DialogHeader>
        <div className="max-h-[70vh] overflow-y-auto">
          {loading ? (
            <div className="text-[#888] text-[13px] py-4 text-center">Loading...</div>
          ) : error !== undefined ? (
            <div className="rounded bg-[#2a1a1a] border border-[#c04040] p-2">
              <div className="text-[11px] text-[#c04040] font-semibold mb-1">Error</div>
              <div className="text-[12px] text-[#e0e0e0] break-words">{error}</div>
            </div>
          ) : messages.length === 0 ? (
            <div className="text-[#666] text-[13px] py-4 text-center">No messages</div>
          ) : (
            messages.map((msg, i) => (
              <div key={i} className="mb-3">
                <div className="flex items-center gap-2 mb-1">
                  <span
                    className="text-[10px] font-bold uppercase px-1.5 py-0.5 rounded"
                    style={{ color: roleColor(msg.role), background: '#2a2a44' }}
                  >
                    {msg.role}
                  </span>
                </div>
                <div className="bg-[#12121e] border border-[#2a2a44] rounded p-2 max-h-[300px] overflow-y-auto">
                  <pre className="text-[12px] text-[#ccc] font-mono whitespace-pre-wrap break-words">
                    {msg.content}
                  </pre>
                </div>
              </div>
            ))
          )}
        </div>
      </DialogContent>
    </Dialog>
  )
}
