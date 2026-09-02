// frontend/src/stores/tasks.ts
// Tasks tab state: the task list (hydrated from the per-node cache), the
// status filter chip selection, the expanded-row selection, and loading flag.
// The per-node cache itself lives in stores/cache.ts (nodeDataCacheAtom, key
// "tasks") — TasksPanel writes/reads it around tasksAtom.
import { atom } from 'jotai'
import type { TaskEntry } from '@/types'
export const tasksAtom = atom<TaskEntry[]>([])
export const statusFilterAtom = atom<string>('all')
export const selectedTaskIdAtom = atom<string | null>(null)
export const tasksLoadingAtom = atom(false)
