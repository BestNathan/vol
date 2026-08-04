// frontend/src/stores/tools.ts
// Tools tab state: call history fed by event handlers, the cached system tool
// list (`tool.list`), and a loading flag.
import { atom } from 'jotai'
import type { ToolCallEntry } from '@/types'

export const toolCallsAtom = atom<ToolCallEntry[]>([])
export const systemToolsAtom = atom<{ name: string; description: string; parameters?: unknown }[]>([])
export const toolsLoadingAtom = atom(false)
