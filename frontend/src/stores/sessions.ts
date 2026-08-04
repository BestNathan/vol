// frontend/src/stores/sessions.ts
// Sessions sub-tab state: the persisted session list for the currently
// selected agent, plus its fetch lifecycle flags.
import { atom } from 'jotai'
import type { SessionListEntry } from '@/types'

export const sessionsAtom = atom<SessionListEntry[]>([])
export const sessionsLoadingAtom = atom(false)
export const sessionsErrorAtom = atom<string | null>(null)
