// frontend/src/components/panels/SkillsPanel.tsx
// Skills panel: desktop table / mobile cards of discovered skills, port of
// skills.rs. Fetches skill.list on mount and whenever the active node changes
// (stale-response guard drops responses that arrive after a node switch),
// opens SkillDetailDialog via skillDialogAtom when a skill is clicked, and
// re-discovers skills via skill.refresh + re-list on Refresh.
import { useCallback, useEffect, useRef } from 'react'
import { useAtom, useAtomValue } from 'jotai'
import { getPanelClient } from '@/lib/panel-client'
import { skillsAtom, skillsErrorAtom, skillsLoadingAtom } from '@/stores/skills'
import { skillDialogAtom } from '@/stores/dialogs'
import { activeNodeIdAtom } from '@/stores/ui'
import { SkillDetailDialog } from '@/components/dialogs/SkillDetailDialog'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Skeleton } from '@/components/ui/skeleton'
import type { RpcMethods } from '@/lib/protocol'
import type { SkillListEntry } from '@/types'

function errMsg(err: unknown): string {
  return (err as { message?: string } | null)?.message ?? String(err)
}

/** Scope badge text color: User green, Repo blue, anything else gold. */
export function scopeColor(scope: string): string {
  switch (scope) {
    case 'User':
      return '#40c040'
    case 'Repo':
      return '#4080ff'
    default:
      return '#c0c040'
  }
}

export function SkillsPanel() {
  const nodeId = useAtomValue(activeNodeIdAtom)
  const [skills, setSkills] = useAtom(skillsAtom)
  const [loading, setLoading] = useAtom(skillsLoadingAtom)
  const [error, setError] = useAtom(skillsErrorAtom)
  const [, setDialog] = useAtom(skillDialogAtom)

  // Live node mirror for the stale-response guard in async callbacks.
  const nodeIdRef = useRef(nodeId)
  useEffect(() => {
    nodeIdRef.current = nodeId
  }, [nodeId])

  // Fetch the skill list; writes are dropped once the active node no longer
  // matches the node this fetch was started for.
  const loadSkills = useCallback(
    async (target: string | null) => {
      if (!target) {
        setSkills([])
        setLoading(false)
        setError(null)
        return
      }
      setLoading(true)
      setError(null)
      try {
        const res = await getPanelClient().call<RpcMethods['skill.list']['result']>('skill.list')
        if (nodeIdRef.current !== target) return
        setSkills(res.skills ?? [])
      } catch (err) {
        if (nodeIdRef.current !== target) return
        setError(errMsg(err))
      } finally {
        if (nodeIdRef.current === target) setLoading(false)
      }
    },
    [setSkills, setLoading, setError],
  )

  // Fetch on mount and whenever the active node changes.
  useEffect(() => {
    void loadSkills(nodeId)
  }, [loadSkills, nodeId])

  // Refresh: re-discover skills via skill.refresh, then re-list.
  const handleRefresh = useCallback(async () => {
    if (!nodeId) return
    try {
      await getPanelClient().call<RpcMethods['skill.refresh']['result']>('skill.refresh')
    } catch (err) {
      setError(errMsg(err))
      return
    }
    await loadSkills(nodeIdRef.current)
  }, [nodeId, loadSkills, setError])

  // Open the detail dialog and fetch full skill details (SKILL.md + files).
  const openSkill = useCallback(
    (skill: SkillListEntry) => {
      setDialog({ open: true, skill: null, loading: true })
      getPanelClient()
        .call<RpcMethods['skill.get']['result']>('skill.get', { name: skill.name })
        .then((res) => {
          setDialog((d) => (d.open ? { ...d, skill: res.skill, loading: false } : d))
        })
        .catch(() => {
          setDialog((d) => (d.open ? { ...d, skill: null, loading: false } : d))
        })
    },
    [setDialog],
  )

  if (!nodeId) {
    return (
      <ScrollArea className="flex-1">
        <div className="h-full p-3 flex items-center justify-center">
          <div className="text-center">
            <div className="text-muted-foreground text-[14px]">Select a node to view skills</div>
            <div className="text-muted-foreground/70 text-[12px] mt-1">
              Select a node from the dropdown above.
            </div>
          </div>
        </div>
      </ScrollArea>
    )
  }

  if (error !== null) {
    return (
      <ScrollArea className="flex-1">
        <div className="h-full p-3 flex items-center justify-center">
          <div className="flex flex-col items-center gap-3 text-center">
            <div className="text-destructive text-[14px]">Failed to load skills</div>
            <div className="text-muted-foreground text-[12px] max-w-[300px] break-words">
              {error}
            </div>
            <Button
              variant="outline"
              size="sm"
              className="cursor-pointer"
              onClick={() => void loadSkills(nodeId)}
            >
              Retry
            </Button>
          </div>
        </div>
      </ScrollArea>
    )
  }

  if (loading && skills.length === 0) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center gap-3 p-4">
        <Skeleton className="h-4 w-48" />
        <Skeleton className="h-4 w-32" />
        <Skeleton className="h-4 w-40" />
      </div>
    )
  }

  return (
    <ScrollArea className="flex-1 min-h-0">
      <div className="h-full p-2.5">
        <div className="flex items-center justify-between mb-2">
          <div className="text-[12px] text-muted-foreground">Skills ({skills.length})</div>
          <Button
            variant="secondary"
            size="sm"
            className="cursor-pointer"
            disabled={loading}
            onClick={() => void handleRefresh()}
          >
            Refresh
          </Button>
        </div>
        {loading && <div className="text-[12px] text-muted-foreground mb-2">Loading...</div>}
        {skills.length === 0 ? (
          <div className="flex items-center justify-center h-32 text-muted-foreground/70 text-[13px]">
            No skills discovered
          </div>
        ) : (
          <>
            {/* Mobile: skill cards */}
            <div className="sm:hidden flex flex-col gap-2">
              {skills.map((s) => (
                <div
                  key={s.id ?? s.name}
                  className="cursor-pointer rounded-md border border-border bg-secondary p-3 active:bg-secondary"
                  onClick={() => openSkill(s)}
                >
                  <div className="flex items-start justify-between gap-3">
                    <div className="min-w-0">
                      <div className="truncate text-[14px] font-bold text-foreground">{s.name}</div>
                      <div className="mt-0.5 text-[11px] text-[#777]">v{s.version}</div>
                    </div>
                    <Badge
                      variant="outline"
                      className="text-[11px] flex-shrink-0"
                      style={{ color: scopeColor(s.scope), borderColor: scopeColor(s.scope) }}
                    >
                      {s.scope}
                    </Badge>
                  </div>
                  {s.description !== '' && (
                    <div className="mt-2 text-[12px] leading-[1.45] text-foreground/70">
                      {s.description}
                    </div>
                  )}
                  {s.triggers.length > 0 && (
                    <div className="flex gap-1 flex-wrap mt-2">
                      {s.triggers.map((t, i) => (
                        <span
                          key={i}
                          className="text-[10px] text-yellow-400/70 bg-[#2a2a20] px-1.5 py-0.5 rounded"
                        >
                          {t}
                        </span>
                      ))}
                    </div>
                  )}
                </div>
              ))}
            </div>
            {/* Desktop: table */}
            <table className="hidden sm:table w-full border-collapse">
              <thead>
                <tr>
                  <th className="text-left px-2 py-1 border-b border-border text-[12px] text-muted-foreground">
                    Name
                  </th>
                  <th className="text-left px-2 py-1 border-b border-border text-[12px] text-muted-foreground">
                    Version
                  </th>
                  <th className="text-left px-2 py-1 border-b border-border text-[12px] text-muted-foreground">
                    Scope
                  </th>
                  <th className="text-left px-2 py-1 border-b border-border text-[12px] text-muted-foreground">
                    Description
                  </th>
                  <th className="text-left px-2 py-1 border-b border-border text-[12px] text-muted-foreground">
                    Triggers
                  </th>
                </tr>
              </thead>
              <tbody>
                {skills.map((s) => (
                  <tr
                    key={s.id ?? s.name}
                    className="cursor-pointer hover:bg-secondary"
                    onClick={() => openSkill(s)}
                  >
                    <td className="px-2 py-1 text-[13px] border-b border-[#2a2a44] text-foreground font-bold">
                      {s.name}
                    </td>
                    <td className="px-2 py-1 text-[13px] border-b border-[#2a2a44] text-muted-foreground">
                      {s.version}
                    </td>
                    <td
                      className="px-2 py-1 text-[13px] border-b border-[#2a2a44]"
                      style={{ color: scopeColor(s.scope) }}
                    >
                      {s.scope}
                    </td>
                    <td className="px-2 py-1 text-[13px] border-b border-[#2a2a44] text-muted-foreground max-w-[260px] truncate">
                      {s.description}
                    </td>
                    <td className="px-2 py-1 border-b border-[#2a2a44]">
                      <div className="flex gap-1 flex-wrap">
                        {s.triggers.map((t, i) => (
                          <span
                            key={i}
                            className="text-[10px] text-yellow-400/70 bg-[#2a2a20] px-1.5 py-0.5 rounded"
                          >
                            {t}
                          </span>
                        ))}
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </>
        )}
        <SkillDetailDialog />
      </div>
    </ScrollArea>
  )
}
