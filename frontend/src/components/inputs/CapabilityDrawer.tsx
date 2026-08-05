// frontend/src/components/inputs/CapabilityDrawer.tsx
// Capability Drawer — fixed right-side panel for selecting agent capabilities
// (tools / skills / MCP servers). Instant-apply toggles: optimistic local
// update → agent.update_capabilities → on success update effective atoms, on
// failure rollback + show warning. Port of capability_drawer.rs.
import { useCallback, useEffect, useState } from 'react'
import { useAtom, useAtomValue, useStore, type WritableAtom } from 'jotai'
import { getPanelClient } from '@/lib/panel-client'
import { cn } from '@/lib/utils'
import { selectedAgentIdAtom } from '@/stores/agents'
import { sessionIdAtom } from '@/stores/connection'
import {
  capOverlayAtom, drawerOpenAtom, drawerSearchAtom, savingStatesAtom,
  selectedToolsAtom, selectedSkillsAtom, selectedMcpsAtom,
} from '@/stores/capability'
import type { GetCapabilitiesResult, UpdateCapabilitiesResult } from '@/lib/protocol'
import type { ToggleSavingState } from '@/types'

type CapGroup = 'tools' | 'skills' | 'mcps'

const GROUP_ATOMS: Record<CapGroup, WritableAtom<Set<string>, [Set<string>], unknown>> = {
  tools: selectedToolsAtom,
  skills: selectedSkillsAtom,
  mcps: selectedMcpsAtom,
}

// Extract {name, isBase} pairs from the available_* item lists, filtered by
// the drawer search (case-insensitive substring on the item name).
export function filterCapabilityItems(
  items: unknown[],
  base: string[],
  search: string,
): { name: string; isBase: boolean }[] {
  const searchLower = search.trim().toLowerCase()
  const baseSet = new Set(base)
  const out: { name: string; isBase: boolean }[] = []
  for (const item of items) {
    if (item && typeof item === 'object') {
      const name = (item as Record<string, unknown>).name
      if (
        typeof name === 'string'
        && name !== ''
        && (searchLower === '' || name.toLowerCase().includes(searchLower))
      ) {
        out.push({ name, isBase: baseSet.has(name) })
      }
    }
  }
  return out
}

export function CapabilityDrawer() {
  const open = useAtomValue(drawerOpenAtom)
  const selectedAgentId = useAtomValue(selectedAgentIdAtom)
  const sessionId = useAtomValue(sessionIdAtom)
  const store = useStore()
  const [overlay, setOverlay] = useAtom(capOverlayAtom)
  const [search, setSearch] = useAtom(drawerSearchAtom)
  const [savingStates, setSavingStates] = useAtom(savingStatesAtom)
  const selectedTools = useAtomValue(selectedToolsAtom)
  const selectedSkills = useAtomValue(selectedSkillsAtom)
  const selectedMcps = useAtomValue(selectedMcpsAtom)

  // Local per-open-session state: fetch done, fetch error, collapsed sections.
  const [loaded, setLoaded] = useState(false)
  const [loadError, setLoadError] = useState<string | null>(null)
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set())

  // Load capabilities when the drawer opens; refetch if the agent or session
  // changes while open. On close, reset per-open state so the next open
  // fetches fresh data.
  useEffect(() => {
    if (!open) {
      setLoaded(false)
      setLoadError(null)
      setSavingStates({})
      return
    }
    if (!selectedAgentId) return
    setLoaded(false)
    setLoadError(null)
    let stale = false
    getPanelClient()
      .call<GetCapabilitiesResult>('agent.get_capabilities', {
        agent_id: selectedAgentId,
        session_id: sessionId,
      })
      .then((res) => {
        if (stale) return
        setOverlay((o) => ({
          ...o,
          effective_tools: res.effective_tools,
          effective_skills: res.effective_skills,
          effective_mcp_servers: res.effective_mcp_servers,
          available_tools: res.available_tools,
          available_skills: res.available_skills,
          available_mcp_servers: res.available_mcp_servers,
          base_tools: res.base_tools,
          base_skills: res.base_skills,
          base_mcp_servers: res.base_mcp_servers,
          loading: false,
        }))
        // Initialize the draft selection sets from the effective lists.
        store.set(selectedToolsAtom, new Set(res.effective_tools))
        store.set(selectedSkillsAtom, new Set(res.effective_skills))
        store.set(selectedMcpsAtom, new Set(res.effective_mcp_servers))
        setLoaded(true)
      })
      .catch((err) => {
        if (stale) return
        const message = (err as { message?: string } | null)?.message ?? String(err)
        setLoadError(message)
        setLoaded(true)
        setOverlay((o) => ({ ...o, loading: false }))
      })
    return () => { stale = true }
  }, [open, selectedAgentId, sessionId, setOverlay, setSavingStates, store])

  // Instant-apply toggle: optimistic local update, then RPC; on success write
  // the server's effective lists back to the overlay, on failure rollback and
  // surface the error. Stale responses (item toggled again mid-flight) are
  // discarded so an older response cannot clobber newer state.
  const handleToggle = useCallback((group: CapGroup, name: string, enabled: boolean) => {
    if (!selectedAgentId) return
    const key = `${group}:${name}`
    const selAtom = GROUP_ATOMS[group]

    const optimistic = new Set(store.get(selAtom))
    if (enabled) optimistic.add(name)
    else optimistic.delete(name)
    store.set(selAtom, optimistic)

    setSavingStates((prev) => ({ ...prev, [key]: { kind: 'saving' } }))

    const effective_tools = [...store.get(selectedToolsAtom)]
    const effective_skills = [...store.get(selectedSkillsAtom)]
    const effective_mcp_servers = [...store.get(selectedMcpsAtom)]

    getPanelClient()
      .call<UpdateCapabilitiesResult>('agent.update_capabilities', {
        agent_id: selectedAgentId,
        session_id: sessionId,
        effective_tools,
        effective_skills,
        effective_mcp_servers,
      })
      .then((res) => {
        // Race guard: user toggled this item again while in flight.
        if (store.get(selAtom).has(name) !== enabled) return
        setOverlay((o) => ({
          ...o,
          effective_tools: res.effective_tools,
          effective_skills: res.effective_skills,
          effective_mcp_servers: res.effective_mcp_servers,
        }))
        setSavingStates((prev) => ({ ...prev, [key]: { kind: 'saved' } }))
        // Checkmark ages out after 1.5s.
        window.setTimeout(() => {
          setSavingStates((prev) => {
            if (prev[key]?.kind !== 'saved') return prev
            const next = { ...prev }
            delete next[key]
            return next
          })
        }, 1500)
      })
      .catch((err) => {
        if (store.get(selAtom).has(name) !== enabled) return
        // Rollback the optimistic update.
        const rollback = new Set(store.get(selAtom))
        if (enabled) rollback.delete(name)
        else rollback.add(name)
        store.set(selAtom, rollback)
        const message = (err as { message?: string } | null)?.message ?? String(err)
        setSavingStates((prev) => ({ ...prev, [key]: { kind: 'error', message } }))
      })
  }, [selectedAgentId, sessionId, setOverlay, setSavingStates, store])

  const closeDrawer = useCallback(() => {
    store.set(drawerOpenAtom, false)
  }, [store])

  const toggleCollapsed = useCallback((section: string) => {
    setCollapsed((prev) => {
      const next = new Set(prev)
      if (next.has(section)) next.delete(section)
      else next.add(section)
      return next
    })
  }, [])

  if (!open) return null

  const sections: {
    group: CapGroup
    title: string
    items: unknown[]
    base: string[]
    selected: Set<string>
  }[] = [
    { group: 'tools', title: 'Tools', items: overlay.available_tools, base: overlay.base_tools, selected: selectedTools },
    { group: 'skills', title: 'Skills', items: overlay.available_skills, base: overlay.base_skills, selected: selectedSkills },
    { group: 'mcps', title: 'MCP Servers', items: overlay.available_mcp_servers, base: overlay.base_mcp_servers, selected: selectedMcps },
  ]

  return (
    <>
      {/* Backdrop overlay */}
      <div className="fixed inset-0 bg-black/50 z-40" onClick={closeDrawer} />

      {/* Drawer panel — full width on mobile, fixed 320px right panel on desktop */}
      <div className="fixed right-0 top-0 h-full w-full sm:w-80 bg-background border-l border-border z-50 flex flex-col shadow-2xl">
        {/* Header */}
        <div className="flex items-center justify-between px-3 py-3 border-b border-border flex-shrink-0">
          <span className="text-[14px] font-semibold text-foreground pl-1">Capabilities</span>
          <button
            type="button"
            onClick={closeDrawer}
            aria-label="Close capabilities drawer"
            className="text-[18px] text-muted-foreground hover:text-foreground/80 leading-none pr-1 cursor-pointer"
          >
            ✕
          </button>
        </div>

        <div className="flex-1 overflow-y-auto">
          {!selectedAgentId ? (
            <div className="p-4 text-muted-foreground text-[13px] text-center">No agent selected</div>
          ) : !loaded ? (
            <div className="p-4 text-muted-foreground text-[13px] text-center">Loading...</div>
          ) : loadError ? (
            <div className="p-4 text-destructive text-[13px] text-center">Error: {loadError}</div>
          ) : (
            <>
              {/* Search */}
              <div className="px-3 py-2">
                <div className="relative">
                  <span className="absolute inset-y-0 left-3 flex items-center text-muted-foreground/70 text-[12px] pointer-events-none">
                    🔍
                  </span>
                  <input
                    type="text"
                    value={search}
                    onChange={(e) => setSearch(e.target.value)}
                    placeholder="Search capabilities..."
                    className="w-full pl-8 pr-2 py-1.5 bg-[#12121e] border border-[#2a2a44] rounded text-[16px] sm:text-[12px] text-foreground/80 placeholder:text-muted-foreground/60 focus:outline-none focus:border-primary"
                  />
                </div>
              </div>

              {/* Capability groups */}
              {sections.map((section, i) => (
                <div key={section.group}>
                  {i > 0 && <div className="border-t border-[#2a2a44] my-1" />}
                  <SectionGroup
                    title={section.title}
                    group={section.group}
                    items={section.items}
                    base={section.base}
                    selected={section.selected}
                    savingStates={savingStates}
                    collapsed={collapsed}
                    search={search}
                    onToggle={handleToggle}
                    onCollapse={() => toggleCollapsed(section.title)}
                  />
                </div>
              ))}
            </>
          )}
        </div>
      </div>
    </>
  )
}

// --- Sub-components -----------------------------------------------------------

function SectionGroup({
  title, group, items, base, selected, savingStates, collapsed, search,
  onToggle, onCollapse,
}: {
  title: string
  group: CapGroup
  items: unknown[]
  base: string[]
  selected: Set<string>
  savingStates: Record<string, ToggleSavingState>
  collapsed: Set<string>
  search: string
  onToggle: (group: CapGroup, name: string, enabled: boolean) => void
  onCollapse: () => void
}) {
  const isCollapsed = collapsed.has(title)
  const filtered = filterCapabilityItems(items, base, search)

  return (
    <div className="px-3 py-1">
      {/* Header row */}
      <button
        type="button"
        onClick={onCollapse}
        className="flex items-center justify-between w-full hover:bg-secondary rounded px-1 py-1 cursor-pointer"
      >
        <span className="text-[11px] font-semibold text-muted-foreground uppercase tracking-[0.5px]">
          {title} ({filtered.length})
        </span>
        <span className="text-[10px] text-muted-foreground/70">{isCollapsed ? '▸' : '▾'}</span>
      </button>

      {/* Items */}
      {!isCollapsed && (
        filtered.length === 0 ? (
          <div className="text-[11px] text-muted-foreground/70 px-1 py-1 w-full">No matching capabilities</div>
        ) : (
          filtered.map(({ name, isBase }) => (
            <CapabilityToggle
              key={name}
              name={name}
              isBase={isBase}
              checked={selected.has(name)}
              savingState={savingStates[`${group}:${name}`]}
              onToggle={() => onToggle(group, name, !selected.has(name))}
            />
          ))
        )
      )}
    </div>
  )
}

function CapabilityToggle({
  name, isBase, checked, savingState, onToggle,
}: {
  name: string
  isBase: boolean
  checked: boolean
  savingState: ToggleSavingState | undefined
  onToggle: () => void
}) {
  return (
    <div className="flex items-center gap-2 py-1 px-1 hover:bg-secondary/50 rounded w-full">
      {/* Toggle switch */}
      <button
        type="button"
        onClick={onToggle}
        role="switch"
        aria-checked={checked}
        aria-label={name}
        className={cn(
          'inline-flex w-8 h-4 rounded-full relative transition-colors flex-shrink-0 border-0 p-0 cursor-pointer',
          checked ? 'bg-[#4080ff]' : 'bg-[#3a3a55]'
        )}
      >
        <span
          className={cn(
            'absolute top-[2px] w-3 h-3 rounded-full transition-all',
            checked ? 'right-[2px] bg-white' : 'left-[2px] bg-[#888]'
          )}
        />
      </button>
      {/* Name — blue when NOT in the base list (a capability the user added) */}
      <span className={cn('text-[12px] flex-1 truncate', isBase ? 'text-foreground' : 'text-primary')}>
        {name}
      </span>
      {/* Saving feedback: spinner → checkmark → (ages out) / error */}
      {savingState?.kind === 'saving' && (
        <span className="text-[#c0a040] text-[12px] animate-spin flex-shrink-0 inline-block" aria-label={`Saving ${name}`}>◌</span>
      )}
      {savingState?.kind === 'saved' && (
        <span className="text-emerald-400 text-[12px] flex-shrink-0" aria-label={`Saved ${name}`}>✓</span>
      )}
      {savingState?.kind === 'error' && (
        <span className="text-destructive text-[12px] cursor-help flex-shrink-0" title={savingState.message} aria-label={`Error: ${savingState.message}`}>⚠</span>
      )}
    </div>
  )
}
