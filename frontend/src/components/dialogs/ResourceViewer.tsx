// frontend/src/components/dialogs/ResourceViewer.tsx
// MCP resource viewer: URI header, a Read button that calls
// `mcp.read_resource({ uri })`, and the content in a pre block (or an error
// box). Atom-driven via mcpDialogAtom.resourceViewer. Port of
// mcp_resource_viewer.rs on the shadcn Dialog.
import { useAtom } from 'jotai'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { mcpDialogAtom } from '@/stores/dialogs'
import { getPanelClient } from '@/lib/panel-client'
import type { RpcMethods } from '@/lib/protocol'

function errMsg(err: unknown): string {
  return (err as { message?: string } | null)?.message ?? String(err)
}

export function ResourceViewer() {
  const [dialog, setDialog] = useAtom(mcpDialogAtom)
  const d = dialog.resourceViewer
  const open = d !== null

  const close = () => setDialog((s) => ({ ...s, resourceViewer: null }))

  const handleRead = async () => {
    if (!d) return
    setDialog((s) =>
      s.resourceViewer
        ? { ...s, resourceViewer: { ...s.resourceViewer, loading: true, error: undefined } }
        : s,
    )
    try {
      const res = await getPanelClient().call<RpcMethods['mcp.read_resource']['result']>(
        'mcp.read_resource',
        {
          uri: d.uri,
        },
      )
      setDialog((s) =>
        s.resourceViewer
          ? { ...s, resourceViewer: { ...s.resourceViewer, content: res.content, loading: false } }
          : s,
      )
    } catch (err) {
      setDialog((s) =>
        s.resourceViewer
          ? { ...s, resourceViewer: { ...s.resourceViewer, error: errMsg(err), loading: false } }
          : s,
      )
    }
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next) close()
      }}
    >
      <DialogContent className="sm:max-w-[600px]">
        <DialogHeader>
          <DialogTitle className="truncate">{d?.uri ?? ''}</DialogTitle>
          <DialogDescription>MCP resource</DialogDescription>
        </DialogHeader>
        <div className="max-h-[60vh] overflow-y-auto flex flex-col gap-3">
          {/* Read stays visible after an error so the user can retry. */}
          {d && !d.loading && d.content === undefined && (
            <Button size="sm" onClick={() => void handleRead()}>
              Read
            </Button>
          )}
          {d?.loading && <div className="text-[13px] text-muted-foreground">Loading...</div>}
          {d?.content !== undefined && (
            <div className="rounded bg-card border border-border p-2">
              <pre className="text-[12px] text-foreground font-mono whitespace-pre-wrap break-all overflow-x-auto">
                {d.content}
              </pre>
            </div>
          )}
          {d?.error !== undefined && (
            <div className="rounded bg-red-950/30 border border-destructive/50 p-2">
              <div className="text-[11px] text-destructive font-semibold mb-1">Error</div>
              <div className="text-[12px] text-foreground break-words">{d.error}</div>
            </div>
          )}
        </div>
      </DialogContent>
    </Dialog>
  )
}
