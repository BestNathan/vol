// frontend/src/stores/mcp.ts
// MCP panel state: the five node-cached lists (servers, tools, resources,
// resource templates, prompts) plus loading/error, and the active sub-tab.
import { atom } from 'jotai'
import type {
  McpServerInfo,
  McpToolInfo,
  McpResourceInfo,
  McpResourceTemplateInfo,
  McpPromptInfo,
  McpSubtab,
} from '@/types'

export interface McpState {
  servers: McpServerInfo[]
  tools: McpToolInfo[]
  resources: McpResourceInfo[]
  resourceTemplates: McpResourceTemplateInfo[]
  prompts: McpPromptInfo[]
  loading: boolean
  error: string | null
}

export const mcpStateAtom = atom<McpState>({
  servers: [],
  tools: [],
  resources: [],
  resourceTemplates: [],
  prompts: [],
  loading: true,
  error: null,
})
export const mcpActiveSubtabAtom = atom<McpSubtab>('servers')
