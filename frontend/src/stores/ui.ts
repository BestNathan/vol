// frontend/src/stores/ui.ts
import { atom } from 'jotai'
import type { ActiveTab } from '@/types'

export const activeTabAtom = atom<ActiveTab>('agents')
export const viewingNodeDetailAtom = atom<string | null>(null)
export const activeNodeIdAtom = atom<string | null>(null)
/** Synthetic node ID used in DataPlane-only mode, where the main connection
 * IS the data plane and agent.* RPCs need no per-node routing. */
export const LOCAL_NODE_ID = 'local'
export const fileTreeDrawerOpenAtom = atom(false)
