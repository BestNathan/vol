// frontend/src/components/dialogs/McpToolDialog.tsx
// MCP tool call dialog: SchemaForm over the tool's input_schema, a Call
// button, and a result/error display. Atom-driven via mcpDialogAtom — the
// McpPanel opens it by setting `toolCallDialog`; the RPC is mcp.call_tool.
// Mirrors mcp_tool_dialog.rs, built on the shadcn Dialog like ToolCallDialog.
import { useEffect, useState } from 'react'
import { useAtom } from 'jotai'
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { SchemaForm } from '@/components/inputs/SchemaForm'
import { Button } from '@/components/ui/button'
import { ScrollArea } from '@/components/ui/scroll-area'
import { mcpDialogAtom } from '@/stores/dialogs'
import { getPanelClient } from '@/lib/panel-client'
import type { RpcMethods } from '@/lib/protocol'

/**
 * Extract displayable text from an `mcp.call_tool` result envelope. The
 * backend serializes the MCP call result as a JSON string (`result`), so a
 * string passes through verbatim; anything else is pretty-printed.
 */
export function formatMcpCallResult(result: { tool_name: string; result: unknown }): string {
  const r = result?.result
  if (typeof r === 'string') return r
  if (r === undefined || r === null) return ''
  try {
    return JSON.stringify(r, null, 2)
  } catch {
    return String(r)
  }
}

function asRecord(v: unknown): Record<string, unknown> | undefined {
  return typeof v === 'object' && v !== null && !Array.isArray(v)
    ? (v as Record<string, unknown>)
    : undefined
}

function errMsg(err: unknown): string {
  return (err as { message?: string } | null)?.message ?? String(err)
}

export function McpToolDialog() {
  const [dialog, setDialog] = useAtom(mcpDialogAtom)
  const d = dialog.toolCallDialog
  const open = d !== null

  // Form values are per-tool local state; result/error/loading live in the
  // atom (reset when the panel opens the dialog). Radix unmounts the content
  // on close, giving SchemaForm a fresh touched-key set per opening.
  const [value, setValue] = useState<Record<string, unknown>>({})
  useEffect(() => {
    if (!open) return
    setValue({})
  }, [open, d?.server, d?.toolName])

  const close = () => setDialog((s) => ({ ...s, toolCallDialog: null }))

  const handleCall = async () => {
    if (!d) return
    setDialog((s) =>
      s.toolCallDialog
        ? { ...s, toolCallDialog: { ...s.toolCallDialog, loading: true, error: undefined, result: undefined } }
        : s
    )
    try {
      const res = await getPanelClient().call<RpcMethods['mcp.call_tool']['result']>('mcp.call_tool', {
        server: d.server,
        tool_name: d.toolName,
        arguments: value,
      })
      setDialog((s) =>
        s.toolCallDialog
          ? { ...s, toolCallDialog: { ...s.toolCallDialog, result: formatMcpCallResult(res), loading: false } }
          : s
      )
    } catch (err) {
      setDialog((s) =>
        s.toolCallDialog
          ? { ...s, toolCallDialog: { ...s.toolCallDialog, error: errMsg(err), loading: false } }
          : s
      )
    }
  }

  return (
    <Dialog open={open} onOpenChange={(next) => { if (!next) close() }}>
      <DialogContent className="sm:max-w-[600px] w-[95vw] max-h-[85vh] overflow-hidden flex flex-col">
        <DialogHeader>
          <DialogTitle className="truncate pr-8">{d ? `${d.server} / ${d.toolName}` : ''}</DialogTitle>
        </DialogHeader>
        <ScrollArea className="flex-1 min-h-0">
          <div className="space-y-3 p-1">
            <SchemaForm
              key={d?.toolName ?? 'closed'}
              schema={asRecord(d?.inputSchema) ?? {}}
              value={value}
              onChange={setValue}
            />
          </div>
        </ScrollArea>
        <DialogFooter className="flex-col gap-2 sm:flex-row sm:justify-between items-stretch">
          <Button size="sm" onClick={() => void handleCall()} disabled={d?.loading}>
            {d?.loading ? 'Calling...' : 'Call'}
          </Button>
          {d?.result !== undefined && d.result !== '' && (
            <div className="rounded bg-emerald-950/30 border border-emerald-500/50 p-2 min-w-0 flex-1">
              <div className="text-xs text-emerald-400 font-semibold mb-1">Result</div>
              <pre className="text-xs text-foreground font-mono whitespace-pre-wrap break-words max-h-[200px] overflow-auto">
                {d.result}
              </pre>
            </div>
          )}
          {d?.error !== undefined && (
            <div className="rounded bg-red-950/30 border border-destructive/50 p-2 min-w-0 flex-1">
              <div className="text-xs text-destructive font-semibold mb-1">Error</div>
              <div className="text-xs text-foreground break-words">{d.error}</div>
            </div>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
