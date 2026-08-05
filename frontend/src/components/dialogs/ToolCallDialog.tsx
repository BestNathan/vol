// frontend/src/components/dialogs/ToolCallDialog.tsx
// Tool call dialog: SchemaForm over the tool's parameter schema, an Execute
// button, and a result/error display. Port of tool_dialog.rs (SystemToolDialog).
// The RPC is injected via onExecute so the same dialog serves system tools
// (tool.call) and MCP tools (mcp.call_tool, Task 3.3).
import { useLayoutEffect, useState } from 'react'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { SchemaForm } from '@/components/inputs/SchemaForm'
import { Button } from '@/components/ui/button'
import { ScrollArea } from '@/components/ui/scroll-area'

/** Outcome of running a tool: display text on success, message on failure. */
export type ToolCallOutcome = { ok: true; content: string } | { ok: false; error: string }

export interface ToolCallDialogProps {
  open: boolean
  toolName: string
  description?: string
  /** Tool parameter JSON Schema; undefined → "No parameters required". */
  schema?: Record<string, unknown>
  onClose: () => void
  /** Invoked with the collected form values when Execute is clicked. */
  onExecute: (args: Record<string, unknown>) => Promise<ToolCallOutcome>
}

export function ToolCallDialog({
  open,
  toolName,
  description,
  schema,
  onClose,
  onExecute,
}: ToolCallDialogProps) {
  // Form values, result, and error are per-tool: the dialog stays mounted
  // while ToolsTab renders it, so reset whenever a (different) tool opens.
  // Radix unmounts the content on close, which gives SchemaForm a fresh
  // touched-key set per opening.
  const [value, setValue] = useState<Record<string, unknown>>({})
  const [loading, setLoading] = useState(false)
  const [result, setResult] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  useLayoutEffect(() => {
    if (!open) return
    setValue({})
    setLoading(false)
    setResult(null)
    setError(null)
  }, [open, toolName])

  const handleExecute = async () => {
    setLoading(true)
    setResult(null)
    setError(null)
    try {
      const outcome = await onExecute(value)
      if (outcome.ok) {
        setResult(outcome.content)
      } else {
        setError(outcome.error)
      }
    } catch (err) {
      setError((err as { message?: string } | null)?.message ?? String(err))
    } finally {
      setLoading(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={(next) => { if (!next) onClose() }}>
      <DialogContent className="sm:max-w-[600px] w-[95vw] max-h-[85vh] overflow-hidden flex flex-col">
        <DialogHeader>
          <DialogTitle className="truncate pr-8">{toolName}</DialogTitle>
          {description && <DialogDescription className="truncate">{description}</DialogDescription>}
        </DialogHeader>
        <ScrollArea className="flex-1 min-h-0">
          <div className="space-y-3 p-1">
            <SchemaForm
              key={toolName}
              schema={schema ?? {}}
              value={value}
              onChange={setValue}
            />
          </div>
        </ScrollArea>
        <DialogFooter className="flex-col gap-2 sm:flex-row sm:justify-between items-stretch">
          <Button size="sm" onClick={() => void handleExecute()} disabled={loading}>
            {loading ? 'Running...' : 'Execute'}
          </Button>
          {result !== null && (
            <div className="rounded bg-emerald-950/30 border border-emerald-500/50 p-2 min-w-0 flex-1">
              <div className="text-xs text-emerald-400 font-semibold mb-1">Result</div>
              <pre className="text-xs text-foreground font-mono whitespace-pre-wrap break-words max-h-[200px] overflow-auto">
                {result}
              </pre>
            </div>
          )}
          {error !== null && (
            <div className="rounded bg-red-950/30 border border-destructive/50 p-2 min-w-0 flex-1">
              <div className="text-xs text-destructive font-semibold mb-1">Error</div>
              <div className="text-xs text-foreground break-words">{error}</div>
            </div>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
