// frontend/src/components/inputs/CapabilityDrawer.tsx
// Capability Drawer — fixed right-side panel for selecting agent capabilities
// (tools / skills / MCP servers). Instant-apply toggles: optimistic local
// update → agent.update_capabilities → on success update effective atoms, on
// failure rollback + show warning. Port of capability_drawer.rs.
//
// Built on shadcn primitives: Sheet (panel + backdrop + Esc-to-close),
// Input (search), Accordion (collapsible sections), Switch (toggles).
import { useCallback, useEffect, useState } from 'react'
import { useAtom, useAtomValue, useStore, type WritableAtom } from 'jotai'
import { Search } from 'lucide-react'
import { getPanelClient } from '@/lib/panel-client'
import { cn } from '@/lib/utils'
import { selectedAgentIdAtom } from '@/stores/agents'
import { sessionIdAtom } from '@/stores/connection'
import {
  capOverlayAtom,
  drawerOpenAtom,
  drawerSearchAtom,
  savingStatesAtom,
  selectedToolsAtom,
  selectedSkillsAtom,
  selectedMcpsAtom,
} from '@/stores/capability'
import type { GetCapabilitiesResult, UpdateCapabilitiesResult } from '@/lib/protocol'
import type { ToggleSavingState } from '@/types'
import { Sheet, SheetContent, SheetHeader, SheetTitle } from '@/components/ui/sheet'
import { Input } from '@/components/ui/input'
import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from '@/components/ui/accordion'
import { Switch } from '@/components/ui/switch'

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
        typeof name === 'string' &&
        name !== '' &&
        (searchLower === '' || name.toLowerCase().includes(searchLower))
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

  // Local per-open-session state: fetch done, fetch error.
  const [loaded, setLoaded] = useState(false)
  const [loadError, setLoadError] = useState<string | null>(null)

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
    return () => {
      stale = true
    }
  }, [open, selectedAgentId, sessionId, setOverlay, setSavingStates, store])

  // Instant-apply toggle: optimistic local update, then RPC; on success write
  // the server's effective lists back to the overlay, on failure rollback and
  // surface the error. Stale responses (item toggled again mid-flight) are
  // discarded so an older response cannot clobber newer state.
  const handleToggle = useCallback(
    (group: CapGroup, name: string, enabled: boolean) => {
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
    },
    [selectedAgentId, sessionId, setOverlay, setSavingStates, store],
  )

  const closeDrawer = useCallback(() => {
    store.set(drawerOpenAtom, false)
  }, [store])

  const sections: {
    group: CapGroup
    title: string
    items: unknown[]
    base: string[]
    selected: Set<string>
  }[] = [
    {
      group: 'tools',
      title: 'Tools',
      items: overlay.available_tools,
      base: overlay.base_tools,
      selected: selectedTools,
    },
    {
      group: 'skills',
      title: 'Skills',
      items: overlay.available_skills,
      base: overlay.base_skills,
      selected: selectedSkills,
    },
    {
      group: 'mcps',
      title: 'MCP Servers',
      items: overlay.available_mcp_servers,
      base: overlay.base_mcp_servers,
      selected: selectedMcps,
    },
  ]

  return (
    <Sheet
      open={open}
      onOpenChange={(next) => {
        if (!next) closeDrawer()
      }}
    >
      <SheetContent side="right" className="w-full sm:w-80 p-0 flex flex-col">
        {/* Header — Sheet provides the close button */}
        <SheetHeader className="px-3 py-3 border-b border-border flex-shrink-0">
          <SheetTitle className="text-[14px] font-semibold text-foreground">
            Capabilities
          </SheetTitle>
        </SheetHeader>

        {!selectedAgentId ? (
          <div className="p-4 text-muted-foreground text-[13px] text-center">No agent selected</div>
        ) : !loaded ? (
          <div className="p-4 text-muted-foreground text-[13px] text-center">Loading...</div>
        ) : loadError ? (
          <div className="p-4 text-destructive text-[13px] text-center">Error: {loadError}</div>
        ) : (
          <>
            {/* Search */}
            <div className="relative px-3 py-2">
              <Search className="absolute left-6 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-muted-foreground/70 pointer-events-none" />
              <Input
                type="text"
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                placeholder="Search capabilities..."
                className="pl-8 bg-[#12121e] border-[#2a2a44] text-[16px] sm:text-[12px] text-foreground/80 placeholder:text-muted-foreground/60"
              />
            </div>

            {/* Capability groups — collapsible sections (all expanded by default) */}
            <Accordion
              type="multiple"
              defaultValue={sections.map((s) => s.group)}
              className="flex-1 overflow-y-auto"
            >
              {sections.map((section, i) => {
                const filtered = filterCapabilityItems(section.items, section.base, search)
                return (
                  <AccordionItem key={section.group} value={section.group} className="border-0">
                    {i > 0 && <div className="border-t border-[#2a2a44] mx-3" />}
                    <AccordionTrigger className="px-3 py-1 hover:bg-secondary rounded-none text-[11px] font-semibold text-muted-foreground uppercase tracking-[0.5px] hover:no-underline">
                      {section.title} ({filtered.length})
                    </AccordionTrigger>
                    <AccordionContent className="px-3 pb-1">
                      {filtered.length === 0 ? (
                        <div className="text-[11px] text-muted-foreground/70 px-1 py-1">
                          No matching capabilities
                        </div>
                      ) : (
                        filtered.map(({ name, isBase }) => (
                          <CapabilityToggle
                            key={name}
                            name={name}
                            isBase={isBase}
                            checked={section.selected.has(name)}
                            savingState={savingStates[`${section.group}:${name}`]}
                            onToggle={() =>
                              handleToggle(section.group, name, !section.selected.has(name))
                            }
                          />
                        ))
                      )}
                    </AccordionContent>
                  </AccordionItem>
                )
              })}
            </Accordion>
          </>
        )}
      </SheetContent>
    </Sheet>
  )
}

// --- Sub-components -----------------------------------------------------------

function CapabilityToggle({
  name,
  isBase,
  checked,
  savingState,
  onToggle,
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
      <Switch checked={checked} onCheckedChange={onToggle} aria-label={name} />
      {/* Name — blue when NOT in the base list (a capability the user added) */}
      <span
        className={cn('text-[12px] flex-1 truncate', isBase ? 'text-foreground' : 'text-primary')}
      >
        {name}
      </span>
      {/* Saving feedback: spinner → checkmark → (ages out) / error */}
      {savingState?.kind === 'saving' && (
        <span
          className="text-[#c0a040] text-[12px] animate-spin flex-shrink-0 inline-block"
          aria-label={`Saving ${name}`}
        >
          ◌
        </span>
      )}
      {savingState?.kind === 'saved' && (
        <span className="text-emerald-400 text-[12px] flex-shrink-0" aria-label={`Saved ${name}`}>
          ✓
        </span>
      )}
      {savingState?.kind === 'error' && (
        <span
          className="text-destructive text-[12px] cursor-help flex-shrink-0"
          title={savingState.message}
          aria-label={`Error: ${savingState.message}`}
        >
          ⚠
        </span>
      )}
    </div>
  )
}
