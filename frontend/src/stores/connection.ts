// frontend/src/stores/connection.ts
import { atom } from 'jotai'
import type { ConnectionState, ServerType } from '@/types'

export const wsConnectedAtom = atom(false)
export const connectionStateAtom = atom<ConnectionState>('disconnected')
export const serverModeAtom = atom<ServerType>('Unknown')
export const wsUrlAtom = atom('')
export const wsLastErrorAtom = atom<string | null>(null)

// Session + run metrics
export const sessionIdAtom = atom('web-session')
export const runCountAtom = atom(0)
export const iterationAtom = atom(0)
export const toolCallCountAtom = atom(0)
export const runElapsedAtom = atom(0) // ms
export const isRunningAtom = atom(false)
export const unsafeModeAtom = atom(false)
export const exitingAtom = atom(false)

// Per-agent running state
export const runningAgentsAtom = atom<Set<string>>(new Set<string>())
export const runMapAtom = atom<Map<string, string>>(new Map()) // run_id → agent_id
export const pendingSubmitAgentAtom = atom<string | null>(null)
