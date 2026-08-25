// frontend/src/components/layout/TabContent.tsx
import type { ComponentType } from 'react'
import { useAtomValue } from 'jotai'
import { activeTabAtom } from '@/stores/ui'
import { TabsContent } from '@/components/ui/tabs'
import { AgentsPanel } from '@/components/panels/AgentsPanel'
import { ToolsTab } from '@/components/panels/ToolsTab'
import { McpPanel } from '@/components/panels/McpPanel'
import { SandboxesPanel } from '@/components/panels/SandboxesPanel'
import { SkillsPanel } from '@/components/panels/SkillsPanel'
import { TasksPanel } from '@/components/panels/TasksPanel'
import { FileContentView } from '@/components/panels/FileContentView'
import { LogViewer } from '@/components/panels/LogViewer'

const TABS = [
  'tasks',
  'agents',
  'tools',
  'workspace',
  'skills',
  'mcp',
  'sandboxes',
  'logs',
] as const

const PANELS: Record<string, ComponentType> = {
  tasks: TasksPanel,
  agents: AgentsPanel,
  tools: ToolsTab,
  workspace: FileContentView,
  skills: SkillsPanel,
  mcp: McpPanel,
  sandboxes: SandboxesPanel,
  logs: LogViewer,
}

export function TabContent() {
  const active = useAtomValue(activeTabAtom)

  return (
    <div className="flex-1 min-h-0 overflow-hidden flex flex-col">
      {TABS.map((tab) => {
        const Panel = PANELS[tab]
        if (!Panel) return null
        return (
          <TabsContent
            key={tab}
            value={tab}
            forceMount={tab === active ? true : undefined}
            className="flex-1 min-h-0 overflow-hidden mt-0 flex flex-col data-[state=inactive]:hidden"
          >
            {tab === active ? <Panel /> : null}
          </TabsContent>
        )
      })}
    </div>
  )
}
