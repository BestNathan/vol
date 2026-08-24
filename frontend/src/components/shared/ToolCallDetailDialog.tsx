// frontend/src/components/shared/ToolCallDetailDialog.tsx
// Shared dialog for viewing full tool call details: tool name, arguments (formatted JSON),
// and result (markdown rendered). Used by both ConversationView and SessionDetailOverlay.
import { Dialog, DialogContent, DialogTitle } from '@/components/ui/dialog'
import { Markdown } from '@/components/shared/Markdown'

interface ToolResult {
  success: boolean
  fullResult: string
}

interface ToolCallDetailDialogProps {
  open: boolean
  onClose: () => void
  toolName: string
  fullArguments: string
  result?: ToolResult
}

/** Try to pretty-print JSON arguments; fall back to raw string. */
function formatArguments(args: string): string {
  try {
    const parsed = JSON.parse(args)
    return JSON.stringify(parsed, null, 2)
  } catch {
    return args
  }
}

export function ToolCallDetailDialog({
  open,
  onClose,
  toolName,
  fullArguments,
  result,
}: ToolCallDetailDialogProps) {
  const formattedArgs = formatArguments(fullArguments)

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next) onClose()
      }}
    >
      <DialogContent
        overlayClassName="bg-black/50"
        className="w-[95vw] sm:max-w-2xl max-h-[80vh] flex flex-col overflow-hidden"
      >
        <DialogTitle className="text-lg font-bold mb-3 flex items-center gap-2">
          <span className="text-yellow-400 text-sm">[tool]</span>
          <span className="truncate">{toolName}</span>
        </DialogTitle>

        <div className="flex-1 overflow-y-auto flex flex-col gap-4">
          {/* Arguments section */}
          <div className="flex flex-col gap-1.5">
            <div className="text-xs font-semibold text-muted-foreground uppercase tracking-wide">
              Arguments
            </div>
            <pre className="bg-background/50 border border-border/50 rounded-md p-3 text-xs overflow-x-auto whitespace-pre-wrap break-all font-mono">
              {formattedArgs}
            </pre>
          </div>

          {/* Result section */}
          {result && (
            <div className="flex flex-col gap-1.5">
              <div className="text-xs font-semibold text-muted-foreground uppercase tracking-wide flex items-center gap-2">
                <span>Result</span>
                <span
                  className={
                    result.success
                      ? 'text-emerald-400 bg-emerald-950/30 px-1.5 py-0.5 rounded text-[10px] font-bold'
                      : 'text-destructive bg-red-950/30 px-1.5 py-0.5 rounded text-[10px] font-bold'
                  }
                >
                  {result.success ? 'OK' : 'ERR'}
                </span>
              </div>
              <div className="bg-background/50 border border-border/50 rounded-md p-3">
                <Markdown content={result.fullResult} />
              </div>
            </div>
          )}
        </div>
      </DialogContent>
    </Dialog>
  )
}
