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
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { SchemaForm } from '@/components/inputs/SchemaForm'
import { Button } from '@/components/ui/button'
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
      <DialogContent className="sm:max-w-[600px]">
        <DialogHeader>
          <DialogTitle className="truncate">{d ? `${d.server} / ${d.toolName}` : ''}</DialogTitle>
        </DialogHeader>
        <div className="max-h-[60vh] overflow-y-auto space-y-3">
          <SchemaForm
            key={d?.toolName ?? 'closed'}
            schema={asRecord(d?.inputSchema) ?? {}}
            value={value}
            onChange={setValue}
          />
          {d?.loading ? (
            <div className="text-[13px] text-[#888]">Calling...</div>
          ) : (
            <Button size="sm" onClick={() => void handleCall()}>Call</Button>
          )}
          {d?.result !== undefined && d.result !== '' && (
            <div className="rounded bg-[#1a2a1a] border border-[#40c040] p-2">
              <div className="text-[11px] text-[#40c040] font-semibold mb-1">Result</div>
              <pre className="text-[12px] text-[#e0e0e0] font-mono whitespace-pre-wrap break-words overflow-x-auto">
                {d.result}
              </pre>
            </div>
          )}
          {d?.error !== undefined && (
            <div className="rounded bg-[#2a1a1a] border border-[#c04040] p-2">
              <div className="text-[11px] text-[#c04040] font-semibold mb-1">Error</div>
              <div className="text-[12px] text-[#e0e0e0] break-words">{d.error}</div>
            </div>
          )}
        </div>
      </DialogContent>
    </Dialog>
  )
}
