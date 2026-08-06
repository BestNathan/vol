// frontend/src/components/panels/TasksPanel.tsx
// Tasks panel: status-filter chips (all/pending/running/completed) above a
// task list — mobile cards / desktop rows, click-to-expand detail, and a
// "⇄ deps" button that opens the TaskDepGraph modal. Port of tasks_panel.rs.
// Fetches task.list on mount and on node change, caches the result per-node in
// nodeDataCacheAtom (key "tasks") so switching nodes hydrates instantly, and
// re-fetches (invalidating the cache entry) whenever the WS reconnects
// (connectionStateAtom transitions to 'connected').
import { useCallback, useEffect, useRef, useState } from 'react'
import { useAtom, useAtomValue, useSetAtom, useStore } from 'jotai'
import { getPanelClient } from '@/lib/panel-client'
import { getCacheKey, nodeDataCacheAtom } from '@/stores/cache'
import { tasksAtom, statusFilterAtom, selectedTaskIdAtom, tasksLoadingAtom } from '@/stores/tasks'
import { activeNodeIdAtom } from '@/stores/ui'
import { connectionStateAtom } from '@/stores/connection'
import { TaskDepGraph } from '@/components/dialogs/TaskDepGraph'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import type { RpcMethods } from '@/lib/protocol'
import type { TaskEntry } from '@/types'

/** Cache key under which the task list is stored per-node. */
export const TASKS_CACHE_KEY = 'tasks'

/** Status badge/text color, mirroring tasks_panel.rs::status_color. */
export function statusColor(status: string): string {
  switch (status) {
    case 'pending': return '#888'
    case 'running': return '#4080ff'
    case 'completed': return '#40c040'
    case 'failed': return '#ff4040'
    case 'killed': return '#ff8800'
    default: return '#888'
  }
}

function errMsg(err: unknown): string {
  return (err as { message?: string } | null)?.message ?? String(err)
}

const STATUS_FILTERS = ['all', 'pending', 'running', 'completed']

export function TasksPanel() {
  const store = useStore()
  const nodeId = useAtomValue(activeNodeIdAtom)
  const connectionState = useAtomValue(connectionStateAtom)
  const [tasks, setTasks] = useAtom(tasksAtom)
  const [filter, setFilter] = useAtom(statusFilterAtom)
  const [selectedId, setSelectedId] = useAtom(selectedTaskIdAtom)
  const [loading, setLoading] = useAtom(tasksLoadingAtom)
  const setCache = useSetAtom(nodeDataCacheAtom)
  const [error, setError] = useState<string | null>(null)
  const [graphCenter, setGraphCenter] = useState<number | null>(null)

  // Live node mirror for the stale-response guard in async callbacks.
  const nodeIdRef = useRef(nodeId)
  useEffect(() => { nodeIdRef.current = nodeId }, [nodeId])

  // Fetch the task list for `target`; hydrates from the per-node cache when a
  // cached copy exists, otherwise fetches and writes the result back to the
  // cache. Writes are dropped once the active node no longer matches the node
  // this fetch was started for.
  const loadTasks = useCallback(async (target: string | null) => {
    if (!target) {
      setTasks([])
      setLoading(false)
      setError(null)
      return
    }
    const cacheKey = getCacheKey(target, TASKS_CACHE_KEY)
    const cached = store.get(nodeDataCacheAtom).get(cacheKey)?.get('tasks')
    if (Array.isArray(cached)) {
      // Node-cached: hydrate instantly without refetching.
      setTasks(cached as TaskEntry[])
      setLoading(false)
      setError(null)
      return
    }
    setLoading(true)
    setError(null)
    try {
      const res = await getPanelClient().call<RpcMethods['task.list']['result']>('task.list')
      if (nodeIdRef.current !== target) return
      const list = res.tasks ?? []
      setTasks(list)
      setCache((prev) => {
        const next = new Map(prev)
        next.set(cacheKey, new Map<string, unknown>([
          ['tasks', list],
          ['error', null],
          ['loading', false],
        ]))
        return next
      })
    } catch (err) {
      if (nodeIdRef.current !== target) return
      setError(errMsg(err))
    } finally {
      if (nodeIdRef.current === target) setLoading(false)
    }
  }, [setTasks, setLoading, setError, setCache, store])

  // Fetch on mount and whenever the active node changes.
  useEffect(() => {
    void loadTasks(nodeId)
  }, [loadTasks, nodeId])

  // Re-fetch on reconnect: invalidate the node's cache entry, then reload.
  const prevConnRef = useRef(connectionState)
  useEffect(() => {
    const prev = prevConnRef.current
    prevConnRef.current = connectionState
    if (prev === 'connected' || connectionState !== 'connected' || !nodeId) return
    const cacheKey = getCacheKey(nodeId, TASKS_CACHE_KEY)
    setCache((prevCache) => {
      const next = new Map(prevCache)
      next.delete(cacheKey)
      return next
    })
    void loadTasks(nodeIdRef.current)
  }, [connectionState, nodeId, loadTasks, setCache])

  const toggleRow = useCallback((id: number) => {
    setSelectedId((cur) => (cur === id ? null : id))
  }, [setSelectedId])

  const openGraph = useCallback((id: number) => {
    setGraphCenter(id)
  }, [])

  if (!nodeId) {
    return (
      <div className="flex-1 overflow-y-auto p-3 flex items-center justify-center">
        <div className="text-center">
          <div className="text-muted-foreground text-[14px]">Select a node to view tasks</div>
          <div className="text-muted-foreground/70 text-[12px] mt-1">Select a node from the dropdown above.</div>
        </div>
      </div>
    )
  }

  if (error !== null && tasks.length === 0) {
    return (
      <div className="flex-1 overflow-y-auto p-3 flex items-center justify-center">
        <div className="flex flex-col items-center gap-3 text-center">
          <div className="text-destructive text-[14px]">Failed to load tasks</div>
          <div className="text-muted-foreground text-[12px] max-w-[300px] break-words">{error}</div>
          <Button variant="outline" size="sm" className="cursor-pointer" onClick={() => void loadTasks(nodeId)}>Retry</Button>
        </div>
      </div>
    )
  }

  if (loading && tasks.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center gap-2 text-muted-foreground text-[14px]">
        <span className="w-4 h-4 rounded-full border-2 border-border border-t-[#80a0ff] animate-spin" />
        Loading tasks...
      </div>
    )
  }

  const filtered = filter === 'all' ? tasks : tasks.filter((t) => t.status === filter)

  /** Shared row body: id, status badge, subject, assignee, deps button, and the
   *  expandable detail (description + dependency/block tN links). */
  const rowBody = (task: TaskEntry) => {
    const expanded = selectedId === task.id
    const color = statusColor(task.status)
    return (
      <>
        <div className="flex items-center gap-2 min-w-0">
          <span className="text-[11px] text-muted-foreground/60 font-mono whitespace-nowrap">t{task.id}</span>
          <Badge variant="secondary" className="text-[10px] px-1 rounded font-bold whitespace-nowrap"
            style={{ background: color, color: '#1a1a2e' }}>
            {task.status}
          </Badge>
          <span className="text-[13px] text-foreground truncate">{task.subject}</span>
          <div className="flex items-center gap-2 ml-auto flex-shrink-0">
            {task.assignee && (
              <span className="text-[11px] text-muted-foreground/70 whitespace-nowrap">{task.assignee}</span>
            )}
            <Button
              variant="link"
              size="sm"
              className="cursor-pointer text-[11px] px-1 h-auto whitespace-nowrap hover:text-[#a0c0ff]"
              title="View dependency graph"
              onClick={(e) => { e.stopPropagation(); openGraph(task.id) }}
            >
              ⇄ deps
            </Button>
          </div>
        </div>
        {expanded && (
          <div className="mt-2 pl-4 text-[12px] text-foreground/70 flex flex-col gap-1">
            {task.description !== '' && (
              <div className="text-foreground/80 mb-1">{task.description}</div>
            )}
            {task.dependencies.length > 0 && (
              <div className="text-muted-foreground">
                Dependencies:{' '}
                {task.dependencies.map((dep, i) => (
                  <span key={dep}>
                    {i > 0 && ', '}
                    <Button
                      variant="link"
                      size="sm"
                      className="cursor-pointer font-mono h-auto p-0 hover:text-[#a0c0ff]"
                      title="Open dependency graph centered on this task"
                      onClick={(e) => { e.stopPropagation(); openGraph(dep) }}
                    >
                      t{dep}
                    </Button>
                  </span>
                ))}
              </div>
            )}
            {task.blocks.length > 0 && (
              <div className="text-muted-foreground">
                Blocks:{' '}
                {task.blocks.map((blk, i) => (
                  <span key={blk}>
                    {i > 0 && ', '}
                    <Button
                      variant="link"
                      size="sm"
                      className="cursor-pointer font-mono h-auto p-0 hover:text-[#a0c0ff]"
                      title="Open dependency graph centered on this task"
                      onClick={(e) => { e.stopPropagation(); openGraph(blk) }}
                    >
                      t{blk}
                    </Button>
                  </span>
                ))}
              </div>
            )}
          </div>
        )}
      </>
    )
  }

  return (
    <div className="flex-1 flex flex-col min-h-0 overflow-hidden">
      {/* Status filter bar */}
      <div className="flex gap-1 p-2 border-b border-border flex-shrink-0 overflow-x-auto">
        {STATUS_FILTERS.map((label) => {
          const isActive = filter === label
          return (
            <Button
              key={label}
              variant="ghost"
              size="sm"
              className={
                isActive
                  ? 'cursor-pointer px-2 py-0.5 h-auto rounded text-[11px] bg-[#80a0ff] text-[#1a1a2e] hover:bg-[#80a0ff] hover:text-[#1a1a2e] whitespace-nowrap'
                  : 'cursor-pointer px-2 py-0.5 h-auto rounded text-[11px] bg-secondary text-muted-foreground hover:bg-border hover:text-muted-foreground whitespace-nowrap'
              }
              onClick={() => setFilter(label)}
            >
              {label}
            </Button>
          )
        })}
      </div>

      {/* Task list */}
      <div className="flex-1 overflow-y-auto">
        {filtered.length === 0 ? (
          <div className="flex items-center justify-center h-full text-muted-foreground/70 text-[13px]">
            No tasks found
          </div>
        ) : (
          <>
            {/* Mobile: task cards */}
            <div className="sm:hidden flex flex-col gap-2 p-2">
              {filtered.map((task) => (
                <div
                  key={task.id}
                  className="cursor-pointer rounded-md border border-border bg-secondary p-3 active:bg-secondary"
                  style={selectedId === task.id ? { background: '#1a2a44' } : undefined}
                  onClick={() => toggleRow(task.id)}
                >
                  {rowBody(task)}
                </div>
              ))}
            </div>
            {/* Desktop: rows */}
            <div className="hidden sm:block">
              {filtered.map((task) => (
                <div
                  key={task.id}
                  className="p-2 border-b border-border cursor-pointer hover:bg-secondary/50"
                  style={selectedId === task.id ? { background: '#1a2a44' } : undefined}
                  onClick={() => toggleRow(task.id)}
                >
                  {rowBody(task)}
                </div>
              ))}
            </div>
          </>
        )}
      </div>

      {graphCenter !== null && (
        <TaskDepGraph tasks={tasks} centerId={graphCenter} onClose={() => setGraphCenter(null)} />
      )}
    </div>
  )
}
