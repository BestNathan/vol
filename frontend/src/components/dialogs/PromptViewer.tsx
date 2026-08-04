// frontend/src/components/dialogs/PromptViewer.tsx
// MCP prompt viewer: prompt name + server, an arguments JSON textarea
// (pre-filled "{}"), and a Get button that actually calls the RPC —
// `mcp.get_prompt({ name, arguments })` — then shows the formatted prompt
// (markdown) or an error box. Gap fix over the Dioxus reference, whose Get
// button stubbed the call with "mcp.get_prompt not implemented yet".
// Atom-driven via mcpDialogAtom.promptViewer.
import { useAtom } from 'jotai'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Markdown } from '@/components/shared/Markdown'
import { mcpDialogAtom } from '@/stores/dialogs'
import { getPanelClient } from '@/lib/panel-client'
import type { RpcMethods } from '@/lib/protocol'

/**
 * Extract readable text from an MCP content block. The backend serializes
 * message content as JSON (e.g. `[{"type":"text","text":"hello"}]`), so a
 * JSON string is parsed and text blocks joined; plain text passes through.
 */
export function textFromPromptContent(value: unknown): string {
  if (typeof value === 'string') {
    if (value === '') return ''
    try {
      return textFromPromptContent(JSON.parse(value))
    } catch {
      return value
    }
  }
  if (Array.isArray(value)) {
    return value
      .map((part) => {
        if (typeof part === 'string') return part
        if (part && typeof part === 'object') {
          const rec = part as Record<string, unknown>
          if (typeof rec.text === 'string' && rec.text !== '') return rec.text
          if (typeof rec.type === 'string') return rec.type
        }
        return ''
      })
      .filter((s) => s !== '')
      .join('\n')
  }
  if (value && typeof value === 'object') {
    const rec = value as Record<string, unknown>
    if (typeof rec.text === 'string' && rec.text !== '') return rec.text
  }
  return value === undefined || value === null ? '' : JSON.stringify(value)
}

/**
 * Format an `mcp.get_prompt` result (`{ description, messages: [{ role,
 * content }] }`) as markdown text: description paragraph followed by one
 * `### role` section per message. Falls back to pretty-printed JSON when the
 * shape is empty/unexpected so the user always sees something.
 */
export function formatPromptResult(prompt: unknown): string {
  if (typeof prompt === 'string') return prompt
  if (!prompt || typeof prompt !== 'object' || Array.isArray(prompt)) {
    return prompt === undefined ? '' : JSON.stringify(prompt, null, 2)
  }
  const p = prompt as Record<string, unknown>
  const lines: string[] = []
  if (typeof p.description === 'string' && p.description !== '') {
    lines.push(p.description, '')
  }
  if (Array.isArray(p.messages)) {
    for (const m of p.messages) {
      if (!m || typeof m !== 'object') continue
      const rec = m as Record<string, unknown>
      const role = typeof rec.role === 'string' && rec.role !== '' ? rec.role : 'message'
      lines.push(`### ${role}`, '', textFromPromptContent(rec.content), '')
    }
  }
  const out = lines.join('\n').trim()
  return out !== '' ? out : JSON.stringify(prompt, null, 2)
}

function errMsg(err: unknown): string {
  return (err as { message?: string } | null)?.message ?? String(err)
}

export function PromptViewer() {
  const [dialog, setDialog] = useAtom(mcpDialogAtom)
  const d = dialog.promptViewer
  const open = d !== null

  const close = () => setDialog((s) => ({ ...s, promptViewer: null }))

  const handleGet = async () => {
    if (!d) return
    // Parse the arguments textarea; reject anything that is not a JSON object.
    let args: Record<string, unknown>
    try {
      const parsed: unknown = JSON.parse(d.argsJson)
      if (parsed === null || typeof parsed !== 'object' || Array.isArray(parsed)) {
        throw new Error('arguments must be a JSON object')
      }
      args = parsed as Record<string, unknown>
    } catch (err) {
      setDialog((s) =>
        s.promptViewer ? { ...s, promptViewer: { ...s.promptViewer, error: `Invalid JSON: ${errMsg(err)}` } } : s
      )
      return
    }
    setDialog((s) =>
      s.promptViewer
        ? { ...s, promptViewer: { ...s.promptViewer, loading: true, error: undefined, result: undefined } }
        : s
    )
    try {
      const res = await getPanelClient().call<RpcMethods['mcp.get_prompt']['result']>('mcp.get_prompt', {
        name: d.promptName,
        arguments: args,
      })
      setDialog((s) =>
        s.promptViewer
          ? { ...s, promptViewer: { ...s.promptViewer, result: formatPromptResult(res.prompt), loading: false } }
          : s
      )
    } catch (err) {
      setDialog((s) =>
        s.promptViewer ? { ...s, promptViewer: { ...s.promptViewer, error: errMsg(err), loading: false } } : s
      )
    }
  }

  return (
    <Dialog open={open} onOpenChange={(next) => { if (!next) close() }}>
      <DialogContent className="sm:max-w-[600px]">
        <DialogHeader>
          <DialogTitle className="truncate">{d?.promptName ?? ''}</DialogTitle>
          {d && <DialogDescription>Server: {d.server}</DialogDescription>}
        </DialogHeader>
        <div className="max-h-[60vh] overflow-y-auto space-y-3">
          <div className="flex flex-col gap-1">
            <span className="text-[12px] text-[#888]">Arguments (JSON)</span>
            <textarea
              className="w-full h-24 bg-[#252540] border border-[#3a3a55] rounded p-2 text-[16px] sm:text-[12px] text-[#e0e0e0] font-mono resize-none"
              value={d?.argsJson ?? '{}'}
              spellCheck={false}
              onChange={(e) =>
                setDialog((s) =>
                  s.promptViewer ? { ...s, promptViewer: { ...s.promptViewer, argsJson: e.target.value } } : s
                )
              }
            />
          </div>
          {d?.loading ? (
            <div className="text-[13px] text-[#888]">Loading...</div>
          ) : (
            <Button size="sm" onClick={() => void handleGet()}>Get</Button>
          )}
          {d?.result !== undefined && d.result !== '' && (
            <div className="rounded bg-[#1a2a1a] border border-[#40c040] p-2">
              <div className="text-[11px] text-[#40c040] font-semibold mb-1">Result</div>
              <Markdown content={d.result} />
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
