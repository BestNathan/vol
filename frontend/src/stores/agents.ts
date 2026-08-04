// frontend/src/stores/agents.ts
import { atom } from 'jotai'
import type { AgentListEntry, AgentSubTab } from '@/types'

export const agentsAtom = atom<AgentListEntry[]>([])
export const selectedAgentIdAtom = atom<string | null>(null)
export const agentsLoadingAtom = atom(false)
export const agentsErrorAtom = atom<string | null>(null)
export const agentSubTabAtom = atom<AgentSubTab>('conversation')

// Per-agent status: { agentId: { status: 'idle'|'running', runId?: string } }
export const agentStatusMapAtom = atom<Record<string, { status: string; runId?: string }>>({})
