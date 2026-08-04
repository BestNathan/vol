// frontend/src/stores/conversation.ts
import { atom } from 'jotai'
import type { AgentConversation } from '@/types'

// atomFamily equivalent: a derived atom that reads from a Map atom
export const conversationMapAtom = atom<Map<string, AgentConversation>>(new Map())
export const activeAgentIdAtom = atom<string | null>(null)

// Derived: get conversation for a specific agent
export const conversationByAgentAtom = atom((get) => {
  const agentId = get(activeAgentIdAtom)
  if (!agentId) return { entries: [], autoScroll: true } as AgentConversation
  return get(conversationMapAtom).get(agentId) ?? { entries: [], autoScroll: true }
})
