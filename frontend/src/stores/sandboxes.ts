// Sandboxes panel state: list of registered sandboxes + loading/error.
import { atom } from 'jotai'
import type { SandboxInfo } from '@/types'

export interface SandboxesState {
  sandboxes: SandboxInfo[]
  loading: boolean
  error: string | null
}

export const sandboxesStateAtom = atom<SandboxesState>({
  sandboxes: [],
  loading: true,
  error: null,
})
