// frontend/src/stores/ui.ts
import { atom } from 'jotai'
import type { ActiveTab } from '@/types'

export const activeTabAtom = atom<ActiveTab>('agents')
export const viewingNodeDetailAtom = atom<string | null>(null)
export const activeNodeIdAtom = atom<string | null>(null)
export const fileTreeDrawerOpenAtom = atom(false)
