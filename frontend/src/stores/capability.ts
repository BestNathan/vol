// frontend/src/stores/capability.ts
// Capability overlay state: effective/base/available lists served by
// `agent.get_capabilities`, drawer visibility, search, per-toggle saving
// feedback, and the selected (draft) sets applied by instant-apply toggles.
// Mirrors Dioxus CapabilityOverlayState / CapabilityDrawerState (state/mod.rs).
import { atom } from 'jotai'
import type { CapabilityOverlayState, ToggleSavingState } from '@/types'

const EMPTY_OVERLAY: CapabilityOverlayState = {
  effective_tools: [],
  effective_skills: [],
  effective_mcp_servers: [],
  available_tools: [],
  available_skills: [],
  available_mcp_servers: [],
  base_tools: [],
  base_skills: [],
  base_mcp_servers: [],
  loading: false,
  dirty: false,
}

// Shared with CapabilityBar: bar reads effective_* for summary counts; the
// drawer writes effective_* on instant-apply toggle success.
export const capOverlayAtom = atom<CapabilityOverlayState>(EMPTY_OVERLAY)

// CapabilityDrawer visibility (opened from the ✎ button in CapabilityBar)
export const drawerOpenAtom = atom(false)

// Drawer search filter text
export const drawerSearchAtom = atom('')

// Per-toggle instant-apply feedback, keyed `${group}:${name}`
export const savingStatesAtom = atom<Record<string, ToggleSavingState>>({})

// Draft selection sets (initialized from effective on drawer open)
export const selectedToolsAtom = atom<Set<string>>(new Set<string>())
export const selectedSkillsAtom = atom<Set<string>>(new Set<string>())
export const selectedMcpsAtom = atom<Set<string>>(new Set<string>())
