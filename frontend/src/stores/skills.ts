// frontend/src/stores/skills.ts
// Skills panel state: the node-cached skill list plus loading/error flags.
import { atom } from 'jotai'
import type { SkillListEntry } from '@/types'

export const skillsAtom = atom<SkillListEntry[]>([])
export const skillsLoadingAtom = atom(false)
export const skillsErrorAtom = atom<string | null>(null)
