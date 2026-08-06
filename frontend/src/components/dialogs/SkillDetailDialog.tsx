// frontend/src/components/dialogs/SkillDetailDialog.tsx
// Skill detail modal: name/version/scope header, description, trigger chips,
// the SKILL.md body in a scrollable pre block, and a file listing with
// click-to-preview via file.read. Port of skill_detail_dialog.rs, atom-driven
// via skillDialogAtom — the SkillsPanel opens it by setting `open: true`.
import { useEffect, useState } from 'react'
import { useAtom } from 'jotai'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Badge } from '@/components/ui/badge'
import { skillDialogAtom } from '@/stores/dialogs'
import { getPanelClient } from '@/lib/panel-client'
import { scopeColor } from '@/components/panels/SkillsPanel'
import type { RpcMethods } from '@/lib/protocol'

/**
 * Absolute skill file path: `directory + '/' + file`, or the bare file when
 * the directory is empty. Mirrors the abs_path handling in
 * skill_detail_dialog.rs.
 */
export function skillFilePath(directory: string, file: string): string {
  return directory === '' ? file : `${directory}/${file}`
}

function errMsg(err: unknown): string {
  return (err as { message?: string } | null)?.message ?? String(err)
}

// File-preview state: selected file path plus either content or an error.
interface PreviewState {
  path: string
  loading: boolean
  content?: string
  error?: string
}

export function SkillDetailDialog() {
  const [dialog, setDialog] = useAtom(skillDialogAtom)
  const { open, skill, loading } = dialog
  const [preview, setPreview] = useState<PreviewState | null>(null)

  // Reset the file preview whenever a (different) skill opens.
  useEffect(() => {
    setPreview(null)
  }, [skill?.name])

  const close = () => setDialog({ open: false, skill: null, loading: false })

  const readFile = (file: string) => {
    if (!skill) return
    const path = skillFilePath(skill.directory, file)
    setPreview({ path, loading: true })
    getPanelClient().call<RpcMethods['file.read']['result']>('file.read', { path })
      .then((res) => setPreview({ path, loading: false, content: res.content }))
      .catch((err) => setPreview({ path, loading: false, error: errMsg(err) }))
  }

  return (
    <Dialog open={open} onOpenChange={(next) => { if (!next) close() }}>
      <DialogContent className="sm:max-w-[700px]">
        <DialogHeader>
          <DialogTitle className="truncate">
            <div className="flex items-center gap-2 min-w-0">
              <span className="text-[15px] font-semibold text-foreground truncate">{skill?.name ?? ''}</span>
              {skill && (
                <Badge variant="secondary" className="text-[11px] flex-shrink-0">v{skill.version}</Badge>
              )}
              {skill && (
                <Badge variant="outline" className="text-[11px] flex-shrink-0"
                  style={{ color: scopeColor(skill.scope), borderColor: scopeColor(skill.scope) }}>
                  {skill.scope}
                </Badge>
              )}
            </div>
          </DialogTitle>
        </DialogHeader>
        <div className="max-h-[70vh] overflow-y-auto">
          {skill ? (
            <>
              {/* Description */}
              <div className="text-foreground/80 text-[13px] mb-2 mt-2 break-words">{skill.description}</div>

              {/* Triggers */}
              {skill.triggers.length > 0 && (
                <div className="flex gap-1.5 flex-wrap mb-3">
                  {skill.triggers.map((t, i) => (
                    <span key={i} className="text-[11px] text-yellow-400/70 bg-[#2a2a20] px-2 py-0.5 rounded">
                      {t}
                    </span>
                  ))}
                </div>
              )}

              {/* SKILL.md body */}
              <div className="bg-[#12121e] border border-[#2a2a44] rounded p-2 mb-3 max-h-[200px] overflow-y-auto">
                <pre className="text-[12px] text-foreground/70 font-mono whitespace-pre-wrap">{skill.content}</pre>
              </div>

              {/* File listing + preview */}
              {skill.file_listing.length > 0 && (
                <div className="flex flex-col">
                  <div className="text-[11px] text-muted-foreground mb-1 font-semibold">Files</div>
                  <div className="bg-[#12121e] border border-[#2a2a44] rounded max-h-[150px] overflow-y-auto mb-2">
                    {skill.file_listing.map((f) => {
                      const path = skillFilePath(skill.directory, f)
                      const isSelected = preview?.path === path
                      return (
                        <div
                          key={f}
                          className="text-[12px] text-foreground/70 font-mono px-2 py-0.5 border-b border-[#2a2a44] last:border-b-0 cursor-pointer hover:bg-secondary"
                          style={isSelected ? { backgroundColor: '#2a3a4a' } : undefined}
                          onClick={() => readFile(f)}
                        >
                          {f}
                        </div>
                      )
                    })}
                  </div>
                  <div className="border border-[#2a2a44] rounded min-h-[100px] max-h-[250px] overflow-y-auto p-2">
                    {preview === null ? (
                      <div className="text-muted-foreground/70 text-[13px] text-center py-8">Click a file to preview</div>
                    ) : preview.loading ? (
                      <div className="flex items-center gap-2 text-muted-foreground text-[13px]">
                        <span className="text-[11px] text-muted-foreground/70 font-mono">{preview.path}</span>
                        Loading...
                      </div>
                    ) : preview.error !== undefined ? (
                      <div className="text-destructive text-[12px] break-words">{preview.error}</div>
                    ) : (
                      <pre className="text-[12px] text-foreground font-mono whitespace-pre-wrap break-words">
                        {preview.content ?? ''}
                      </pre>
                    )}
                  </div>
                </div>
              )}
            </>
          ) : (
            <div className="text-destructive text-[13px] py-4 text-center">
              {loading ? 'Loading skill details...' : 'Failed to load skill details'}
            </div>
          )}
        </div>
      </DialogContent>
    </Dialog>
  )
}
