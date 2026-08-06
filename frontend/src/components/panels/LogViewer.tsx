// frontend/src/components/panels/LogViewer.tsx
// Log viewer panel: run list (truncated run_id, event count, last event +
// time) → click opens the run's entries with color-coded event types, a
// "← Back to run list" button, and an auto-scroll toggle (on by default).
// Port of crates/vol-llm-ui/src/web/components/log_viewer.rs. Fetches
// log.list on mount / node change and log.read on run click, caching the
// whole viewer state per-node under nodeDataCacheAtom (key "log_viewer") so
// switching nodes restores the exact view instantly; the cache entry is
// invalidated and refetched on WS reconnect (connectionStateAtom).
import { useCallback, useEffect, useRef, useState } from 'react'
import { useAtom, useAtomValue, useSetAtom, useStore } from 'jotai'
import { getPanelClient } from '@/lib/panel-client'
import { getCacheKey, nodeDataCacheAtom } from '@/stores/cache'
import { logRunsAtom, selectedRunAtom, logEntriesAtom, logAutoScrollAtom } from '@/stores/logs'
import { activeNodeIdAtom } from '@/stores/ui'
import { connectionStateAtom } from '@/stores/connection'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import type { RpcMethods } from '@/lib/protocol'
import type { LogLine, LogRunSummary } from '@/types'

/** Cache key under which the whole log viewer state is stored per-node. */
export const LOG_VIEWER_CACHE_KEY = 'log_viewer'

/** Serializable viewer state — mirrors LogViewerCacheState in log_viewer.rs. */
export interface LogViewerCacheState {
  run_logs: LogRunSummary[]
  entries: LogLine[]
  selected_run: string | null
  loading: boolean
  error: string | null
}

/** Guard for a cached LogViewerCacheState (shape-checked, not just truthy). */
export function isLogViewerCacheState(v: unknown): v is LogViewerCacheState {
  if (typeof v !== 'object' || v === null) return false
  const s = v as Record<string, unknown>
  return Array.isArray(s.run_logs) && Array.isArray(s.entries)
}

/** Color for a log event type: AgentStart/AgentComplete green,
 * ToolCallBegin/ToolCallComplete yellow, ToolCallError/AgentAborted red,
 * anything else default grey. Mirrors LogEntryItem in log_viewer.rs. */
export function entryColor(eventType: string): string {
  switch (eventType) {
    case 'AgentStart':
    case 'AgentComplete':
      return '#40c040'
    case 'ToolCallBegin':
    case 'ToolCallComplete':
      return '#c0c040'
    case 'ToolCallError':
    case 'AgentAborted':
      return '#c04040'
    default:
      return '#e0e0e0'
  }
}

/** Truncated run_id for list rows: first 9 chars + "..." when > 12 chars. */
export function shortRunId(runId: string): string {
  return runId.length > 12 ? `${runId.slice(0, 9)}...` : runId
}

function errMsg(err: unknown): string {
  return (err as { message?: string } | null)?.message ?? String(err)
}

export function LogViewer() {
  const store = useStore()
  const nodeId = useAtomValue(activeNodeIdAtom)
  const connectionState = useAtomValue(connectionStateAtom)
  const [runs, setRuns] = useAtom(logRunsAtom)
  const [selectedRun, setSelectedRun] = useAtom(selectedRunAtom)
  const [entries, setEntries] = useAtom(logEntriesAtom)
  const [autoScroll, setAutoScroll] = useAtom(logAutoScrollAtom)
  const setCache = useSetAtom(nodeDataCacheAtom)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  // Live node mirror for the stale-response guard in async callbacks.
  const nodeIdRef = useRef(nodeId)
  useEffect(() => { nodeIdRef.current = nodeId }, [nodeId])

  // Write a partial state patch into the node's "log_viewer" cache entry,
  // preserving the current atoms for any fields not in the patch.
  const writeCache = useCallback((target: string, patch: Partial<LogViewerCacheState>) => {
    const cacheKey = getCacheKey(target, LOG_VIEWER_CACHE_KEY)
    setCache((prev) => {
      const next = new Map(prev)
      const existing = next.get(cacheKey)?.get(LOG_VIEWER_CACHE_KEY)
      const base = isLogViewerCacheState(existing)
        ? existing
        : {
            run_logs: store.get(logRunsAtom),
            entries: store.get(logEntriesAtom),
            selected_run: store.get(selectedRunAtom),
            loading: false,
            error: null,
          }
      next.set(cacheKey, new Map<string, unknown>([[LOG_VIEWER_CACHE_KEY, { ...base, ...patch }]]))
      return next
    })
  }, [setCache, store])

  // Fetch the run list for `target`; hydrates from the per-node cache when a
  // cached copy exists (restoring the selected run + entries too), otherwise
  // fetches and writes the result back to the cache. Writes are dropped once
  // the active node no longer matches the node this fetch was started for.
  const loadRuns = useCallback(async (target: string | null) => {
    if (!target) {
      setRuns([])
      setEntries([])
      setSelectedRun(null)
      setLoading(false)
      setError(null)
      return
    }
    const cacheKey = getCacheKey(target, LOG_VIEWER_CACHE_KEY)
    const cached = store.get(nodeDataCacheAtom).get(cacheKey)?.get(LOG_VIEWER_CACHE_KEY)
    if (isLogViewerCacheState(cached)) {
      setRuns(cached.run_logs)
      setEntries(cached.entries)
      setSelectedRun(cached.selected_run)
      setLoading(cached.loading)
      setError(cached.error)
      return
    }
    setLoading(true)
    setError(null)
    try {
      const res = await getPanelClient().call<RpcMethods['log.list']['result']>('log.list')
      if (nodeIdRef.current !== target) return
      const list = res.runs ?? []
      setRuns(list)
      writeCache(target, { run_logs: list, loading: false, error: null })
    } catch (err) {
      if (nodeIdRef.current !== target) return
      setError(errMsg(err))
    } finally {
      if (nodeIdRef.current === target) setLoading(false)
    }
  }, [setRuns, setEntries, setSelectedRun, setLoading, setError, writeCache, store])

  // Fetch on mount and whenever the active node changes.
  useEffect(() => {
    void loadRuns(nodeId)
  }, [loadRuns, nodeId])

  // Re-fetch on reconnect: invalidate the node's cache entry, then reload.
  const prevConnRef = useRef(connectionState)
  useEffect(() => {
    const prev = prevConnRef.current
    prevConnRef.current = connectionState
    if (prev === 'connected' || connectionState !== 'connected' || !nodeId) return
    const cacheKey = getCacheKey(nodeId, LOG_VIEWER_CACHE_KEY)
    setCache((prevCache) => {
      const next = new Map(prevCache)
      next.delete(cacheKey)
      return next
    })
    void loadRuns(nodeIdRef.current)
  }, [connectionState, nodeId, loadRuns, setCache])

  // Open a run: select it, clear the entries, mark loading (in the cache too,
  // mirroring the Dioxus reference), then fetch log.read.
  const openRun = useCallback(async (runId: string) => {
    const target = nodeIdRef.current
    if (!target) return
    setSelectedRun(runId)
    setEntries([])
    setLoading(true)
    setError(null)
    writeCache(target, { selected_run: runId, entries: [], loading: true, error: null })
    try {
      const res = await getPanelClient().call<RpcMethods['log.read']['result']>('log.read', { run_id: runId })
      if (nodeIdRef.current !== target) return
      const list = res.entries ?? []
      setEntries(list)
      setLoading(false)
      writeCache(target, { entries: list, loading: false, error: null })
    } catch (err) {
      if (nodeIdRef.current !== target) return
      setError(errMsg(err))
      setLoading(false)
      writeCache(target, { loading: false, error: errMsg(err) })
    }
  }, [setSelectedRun, setEntries, setLoading, setError, writeCache])

  const backToList = useCallback(() => {
    setSelectedRun(null)
    const target = nodeIdRef.current
    if (target) writeCache(target, { selected_run: null })
  }, [setSelectedRun, writeCache])

  // Auto-scroll: while enabled, stick to the bottom as entries arrive; the
  // user scrolling up disengages the stick until it is toggled back on.
  const listRef = useRef<HTMLDivElement>(null)
  const stickRef = useRef(true)
  const programmaticRef = useRef(false)

  const scrollToBottom = useCallback(() => {
    const el = listRef.current
    if (!el) return
    programmaticRef.current = true
    el.scrollTop = el.scrollHeight
    requestAnimationFrame(() => { programmaticRef.current = false })
  }, [])

  const handleScroll = useCallback(() => {
    if (programmaticRef.current) return
    const el = listRef.current
    if (!el) return
    stickRef.current = el.scrollHeight - el.scrollTop - el.clientHeight <= 2
  }, [])

  // Toggling the switch back on re-engages the stick and jumps to the bottom.
  useEffect(() => {
    if (autoScroll) {
      stickRef.current = true
      scrollToBottom()
    }
  }, [autoScroll, scrollToBottom])

  // New entries (or a newly opened run) while auto-scrolling → jump to bottom.
  useEffect(() => {
    if (autoScroll && stickRef.current) scrollToBottom()
  }, [entries, selectedRun, autoScroll, scrollToBottom])

  if (!nodeId) {
    return (
      <div className="flex-1 overflow-y-auto p-3 flex items-center justify-center">
        <div className="text-center">
          <div className="text-muted-foreground text-[14px]">Select a node to view logs</div>
          <div className="text-muted-foreground/70 text-[12px] mt-1">Select a node from the dropdown above.</div>
        </div>
      </div>
    )
  }

  if (error !== null && runs.length === 0) {
    return (
      <div className="flex-1 overflow-y-auto p-3 flex items-center justify-center">
        <div className="flex flex-col items-center gap-3 text-center">
          <div className="text-destructive text-[14px]">Failed to load logs</div>
          <div className="text-muted-foreground text-[12px] max-w-[300px] break-words">{error}</div>
          <Button variant="outline" size="sm" className="cursor-pointer" onClick={() => void loadRuns(nodeId)}>Retry</Button>
        </div>
      </div>
    )
  }

  if (loading && runs.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center gap-2 text-muted-foreground text-[14px]">
        <span className="w-4 h-4 rounded-full border-2 border-border border-t-[#80a0ff] animate-spin" />
        Loading logs...
      </div>
    )
  }

  // ---- Run entries view ----
  if (selectedRun !== null) {
    return (
      <div className="flex-1 flex flex-col min-h-0 overflow-hidden">
        <div className="flex items-center gap-3 p-2 border-b border-border flex-shrink-0">
          <Button variant="link" size="sm" className="cursor-pointer text-[12px] whitespace-nowrap" onClick={backToList}>
            ← Back to run list
          </Button>
          <span className="text-[12px] text-muted-foreground font-mono truncate min-w-0">Log: {selectedRun}</span>
          <label className="ml-auto flex items-center gap-1.5 text-[12px] text-muted-foreground whitespace-nowrap flex-shrink-0 cursor-pointer">
            <Checkbox
              checked={autoScroll}
              onCheckedChange={(checked) => setAutoScroll(checked === true)}
            />
            Auto-scroll
          </label>
        </div>
        {loading && entries.length === 0 ? (
          <div className="flex-1 flex items-center justify-center gap-2 text-muted-foreground text-[14px]">
            <span className="w-4 h-4 rounded-full border-2 border-border border-t-[#80a0ff] animate-spin" />
            Loading log entries...
          </div>
        ) : error !== null && entries.length === 0 ? (
          <div className="flex-1 flex items-center justify-center p-3">
            <div className="text-destructive text-[13px] text-center break-words max-w-[300px]">{error}</div>
          </div>
        ) : entries.length === 0 ? (
          <div className="flex-1 flex items-center justify-center text-muted-foreground/70 text-[13px]">
            No events in this run.
          </div>
        ) : (
          <div
            ref={listRef}
            onScroll={handleScroll}
            className="flex-1 overflow-y-auto p-2.5 font-mono text-[12px]"
          >
            {entries.map((entry, i) => {
              const color = entryColor(entry.event_type)
              return (
                <div key={i} className="py-0.5 whitespace-nowrap">
                  <span className="text-muted-foreground/70">[{entry.timestamp}] </span>
                  <span className="font-bold" style={{ color }}>{entry.event_type}</span>
                  <span style={{ color }}> -- {entry.summary}</span>
                </div>
              )
            })}
          </div>
        )}
      </div>
    )
  }

  // ---- Run list view ----
  return (
    <div className="flex-1 overflow-y-auto p-2.5 font-mono text-[13px]">
      {runs.length === 0 ? (
        <div className="flex items-center justify-center h-full text-muted-foreground/70 text-[13px]">
          No log files found.
        </div>
      ) : (
        <>
          {/* Mobile: run cards */}
          <div className="sm:hidden flex flex-col gap-2">
            {runs.map((run) => (
              <div
                key={run.run_id}
                className="rounded-lg border border-border bg-secondary p-3 cursor-pointer active:bg-secondary"
                onClick={() => void openRun(run.run_id)}
              >
                <div className="flex items-center justify-between gap-2 min-w-0">
                  <span className="text-[#c0c0c0] truncate">{shortRunId(run.run_id)}</span>
                  <span className="text-muted-foreground text-[11px] flex-shrink-0">{run.event_count} events</span>
                </div>
                <div className="mt-1 text-[11px] text-muted-foreground truncate">
                  {run.last_event} ({run.last_event_time})
                </div>
              </div>
            ))}
          </div>
          {/* Desktop: run rows */}
          <div className="hidden sm:block">
            {runs.map((run) => (
              <div
                key={run.run_id}
                className="py-0.5 text-foreground/80 cursor-pointer hover:bg-[#333] flex items-baseline gap-2 min-w-0"
                onClick={() => void openRun(run.run_id)}
              >
                <span className="text-[#c0c0c0] flex-shrink-0">{shortRunId(run.run_id)}</span>
                <span className="text-muted-foreground flex-shrink-0">{run.event_count} events</span>
                <span className="text-muted-foreground truncate">{run.last_event} ({run.last_event_time})</span>
              </div>
            ))}
          </div>
        </>
      )}
    </div>
  )
}
