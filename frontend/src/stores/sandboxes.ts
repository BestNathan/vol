// Sandboxes panel state: spec profiles + running instances + loading/error.
import { atom } from 'jotai'
import type { SandboxInfo, SandboxSpecInfo } from '@/types'

export interface SandboxesState {
  sandboxes: SandboxInfo[]
  loading: boolean
  error: string | null
}

export interface SandboxSpecsState {
  specs: SandboxSpecInfo[]
  loading: boolean
  error: string | null
}

export const sandboxesStateAtom = atom<SandboxesState>({
  sandboxes: [],
  loading: true,
  error: null,
})

export const sandboxSpecsStateAtom = atom<SandboxSpecsState>({
  specs: [],
  loading: true,
  error: null,
})
