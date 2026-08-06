// frontend/src/components/dialogs/ToolDetailDialog.tsx
// Tool detail dialog: left panel (description + parameters) / right panel
// (SchemaForm + Execute + Result). Opens when a tool row is clicked in ToolsTab.
import { useLayoutEffect, useMemo, useState } from 'react'
import { Wrench } from 'lucide-react'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { SchemaForm } from '@/components/inputs/SchemaForm'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { ScrollArea } from '@/components/ui/scroll-area'
import type { SystemTool } from '@/components/panels/ToolsTab'

export type ToolCallOutcome =
  | { ok: true; content: string }
  | { ok: false; error: string }

export interface ToolDetailDialogProps {
  open: boolean
  tool: SystemTool | null
  onClose: () => void
  onExecute: (args: Record<string, unknown>) => Promise<ToolCallOutcome>
}

/** One parsed parameter from the JSON Schema properties. */
interface ParamInfo {
  name: string
  type: string
  description?: string
  required: boolean
}

function asRecord(v: unknown): Record<string, unknown> | undefined {
  return typeof v === 'object' && v !== null && !Array.isArray(v)
    ? (v as Record<string, unknown>)
    : undefined
}

function parseParams(schema: unknown): ParamInfo[] {
  const rec = asRecord(schema)
  if (!rec) return []
  const properties = asRecord(rec.properties)
  if (!properties) return []
  const required = new Set(
    Array.isArray(rec.required) ? rec.required.filter((r): r is string => typeof r === 'string') : [],
  )
  const out: ParamInfo[] = []
  for (const [name, raw] of Object.entries(properties)) {
    const prop = asRecord(raw)
    out.push({
      name,
      type: typeof prop?.type === 'string' ? prop.type : 'string',
      description: typeof prop?.description === 'string' ? prop.description : undefined,
      required: required.has(name),
    })
  }
  return out
}

const TYPE_COLORS: Record<string, string> = {
  string: 'text-sky-400 bg-sky-950/30',
  number: 'text-emerald-400 bg-emerald-950/30',
  integer: 'text-emerald-400 bg-emerald-950/30',
  boolean: 'text-amber-400 bg-amber-950/30',
  object: 'text-purple-400 bg-purple-950/30',
}

export function ToolDetailDialog({
  open,
  tool,
  onClose,
  onExecute,
}: ToolDetailDialogProps) {
  const [value, setValue] = useState<Record<string, unknown>>({})
  const [loading, setLoading] = useState(false)
  const [result, setResult] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  // Reset per tool open
  useLayoutEffect(() => {
    if (!open) return
    setValue({})
    setLoading(false)
    setResult(null)
    setError(null)
  }, [open, tool?.name])

  const params = useMemo(() => parseParams(tool?.parameters), [tool?.parameters])
  const hasParams = params.length > 0

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
      <DialogContent className="sm:max-w-[760px] w-[95vw] h-[92dvh] sm:h-[85vh] overflow-hidden flex flex-col p-0 gap-0">
        {/* Header */}
        <DialogHeader className="px-4 py-3 border-b border-border flex-shrink-0">
          <DialogTitle className="flex items-center gap-2 text-[14px] font-semibold">
            <Wrench className="h-4 w-4 text-muted-foreground" />
            {tool?.name ?? ''}
          </DialogTitle>
        </DialogHeader>

        {/* Body: left (info) + right (run) */}
        <div className="flex flex-col sm:flex-row flex-1 min-h-0 overflow-hidden">
          {/* ── Left panel: Description + Parameters ── */}
          <ScrollArea className="flex-1 min-h-0 sm:min-w-[280px] sm:max-w-[45%] border-b sm:border-b-0 sm:border-r border-border">
            <div className="p-4 space-y-4">
              {/* Description */}
              {tool?.description && (
                <div>
                  <div className="text-[11px] font-semibold text-muted-foreground uppercase tracking-[0.5px] mb-1.5">
                    Description
                  </div>
                  <div className="text-[13px] text-foreground/85 leading-relaxed whitespace-pre-wrap">
                    {tool.description}
                  </div>
                </div>
              )}

              {/* Parameters */}
              {hasParams && (
                <div>
                  <div className="text-[11px] font-semibold text-muted-foreground uppercase tracking-[0.5px] mb-1.5">
                    Parameters ({params.length})
                  </div>
                  <div className="space-y-2">
                    {params.map((p) => (
                      <div key={p.name} className="rounded-md border border-border/70 bg-secondary/30 p-2">
                        <div className="flex items-center gap-1.5 mb-0.5">
                          <code className="text-[12px] font-semibold text-foreground">{p.name}</code>
                          {p.required && (
                            <span className="text-[10px] text-destructive font-semibold">required</span>
                          )}
                          <Badge
                            variant="secondary"
                            className={`text-[10px] px-1 py-0 rounded-[3px] font-medium ${TYPE_COLORS[p.type] ?? 'text-muted-foreground bg-secondary'}`}
                          >
                            {p.type}
                          </Badge>
                        </div>
                        {p.description && (
                          <div className="text-[11px] text-muted-foreground leading-relaxed">
                            {p.description}
                          </div>
                        )}
                      </div>
                    ))}
                  </div>
                </div>
              )}

              {/* No description or params */}
              {!tool?.description && !hasParams && (
                <div className="text-[13px] text-muted-foreground py-8 text-center">
                  No description or parameters
                </div>
              )}
            </div>
          </ScrollArea>

          {/* ── Right panel: SchemaForm + Execute + Result ── */}
          <ScrollArea className="flex-1 min-h-0 sm:min-w-[300px]">
            <div className="p-4 space-y-3">
              <div className="text-[11px] font-semibold text-muted-foreground uppercase tracking-[0.5px]">
                Run
              </div>

              {hasParams ? (
                <SchemaForm
                  key={tool?.name}
                  schema={asRecord(tool?.parameters) ?? {}}
                  value={value}
                  onChange={setValue}
                />
              ) : (
                <div className="text-[12px] text-muted-foreground">No parameters required</div>
              )}

              <Button size="sm" onClick={() => void handleExecute()} disabled={loading}>
                {loading ? 'Running...' : 'Execute'}
              </Button>

              {result !== null && (
                <div className="rounded bg-emerald-950/30 border border-emerald-500/50 p-2.5">
                  <div className="text-[11px] text-emerald-400 font-semibold mb-1 uppercase tracking-[0.3px]">
                    Result
                  </div>
                  <pre className="text-[12px] text-foreground font-mono whitespace-pre-wrap break-words max-h-[300px] overflow-auto">
                    {result}
                  </pre>
                </div>
              )}

              {error !== null && (
                <div className="rounded bg-red-950/30 border border-destructive/50 p-2.5">
                  <div className="text-[11px] text-destructive font-semibold mb-1 uppercase tracking-[0.3px]">
                    Error
                  </div>
                  <div className="text-[12px] text-foreground break-words">{error}</div>
                </div>
              )}
            </div>
          </ScrollArea>
        </div>
      </DialogContent>
    </Dialog>
  )
}
