// frontend/src/components/dialogs/ToolCallDialog.tsx
// Tool call dialog: SchemaForm over the tool's parameter schema, an Execute
// button, and a result/error display. Port of tool_dialog.rs (SystemToolDialog).
// The RPC is injected via onExecute so the same dialog serves system tools
// (tool.call) and MCP tools (mcp.call_tool, Task 3.3).
import { useEffect, useState } from 'react'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { SchemaForm } from '@/components/inputs/SchemaForm'
import { Button } from '@/components/ui/button'

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

  useEffect(() => {
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
      <DialogContent className="sm:max-w-[600px]">
        <DialogHeader>
          <DialogTitle>{toolName}</DialogTitle>
          {description && <DialogDescription className="truncate">{description}</DialogDescription>}
        </DialogHeader>
        <div className="max-h-[60vh] overflow-y-auto space-y-3">
          {/* Fresh form per tool: touched-key state and submitted arguments
              must not leak between tools. */}
          <SchemaForm
            key={toolName}
            schema={schema ?? {}}
            value={value}
            onChange={setValue}
          />
          {loading ? (
            <div className="text-[13px] text-[#888]">Running...</div>
          ) : (
            <Button size="sm" onClick={() => void handleExecute()}>
              Execute
            </Button>
          )}
          {result !== null && (
            <div className="rounded bg-[#1a2a1a] border border-[#40c040] p-2">
              <div className="text-[11px] text-[#40c040] font-semibold mb-1">Result</div>
              <pre className="text-[12px] text-[#e0e0e0] font-mono whitespace-pre-wrap break-words overflow-x-auto">
                {result}
              </pre>
            </div>
          )}
          {error !== null && (
            <div className="rounded bg-[#2a1a1a] border border-[#c04040] p-2">
              <div className="text-[11px] text-[#c04040] font-semibold mb-1">Error</div>
              <div className="text-[12px] text-[#e0e0e0] break-words">{error}</div>
            </div>
          )}
        </div>
      </DialogContent>
    </Dialog>
  )
}
