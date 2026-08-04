// frontend/src/stores/logs.ts
// Log viewer tab state: the run list (hydrated from the per-node cache), the
// selected run, its entry lines, and the auto-scroll toggle. The per-node
// cache itself lives in stores/cache.ts (nodeDataCacheAtom, key "log_viewer")
// — LogViewer writes/reads it around these atoms.
import { atom } from 'jotai'
import type { LogRunSummary, LogLine } from '@/types'
export const logRunsAtom = atom<LogRunSummary[]>([])
export const selectedRunAtom = atom<string | null>(null)
export const logEntriesAtom = atom<LogLine[]>([])
export const logAutoScrollAtom = atom(true)
