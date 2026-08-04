// frontend/src/stores/context.ts
// Context tab state: the selected agent's context contributors
// (agent.context_config) plus the snapshot dialog state
// (agent.context_snapshot). Fetched and driven by ContextPanel.
import { atom } from 'jotai'
import type { ContributorInfoEntry, ContextMessageEntry } from '@/types'

export const contributorsAtom = atom<ContributorInfoEntry[]>([])
export const contextLoadingAtom = atom(false)
export const contextErrorAtom = atom<string | null>(null)

// Snapshot dialog: open flag, the contributor whose snapshot is shown, its
// messages (empty while loading), the loading flag, and an optional error
// (set when agent.context_snapshot fails) for the dialog's error state.
export const contextDialogAtom = atom<{
  open: boolean
  contributorName: string
  messages: ContextMessageEntry[]
  loading: boolean
  error?: string
}>({ open: false, contributorName: '', messages: [], loading: false })
