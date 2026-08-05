// frontend/src/components/layout/TabContent.tsx
import { useAtomValue } from 'jotai'
import { activeTabAtom } from '@/stores/ui'
import { AgentsPanel } from '@/components/panels/AgentsPanel'
import { ToolsTab } from '@/components/panels/ToolsTab'
import { McpPanel } from '@/components/panels/McpPanel'
import { SkillsPanel } from '@/components/panels/SkillsPanel'
import { TasksPanel } from '@/components/panels/TasksPanel'
import { FileContentView } from '@/components/panels/FileContentView'
import { LogViewer } from '@/components/panels/LogViewer'

function PlaceholderPanel({ name }: { name: string }) {
  return (
    <div className="flex items-center justify-center h-full text-muted-foreground/70 text-sm">
      {name} — coming soon
    </div>
  )
}

export function TabContent() {
  const active = useAtomValue(activeTabAtom)

  switch (active) {
    case 'tasks': return <TasksPanel />
    case 'agents': return <AgentsPanel />
    case 'tools': return <ToolsTab />
    case 'workspace': return <FileContentView />
    case 'skills': return <SkillsPanel />
    case 'mcp': return <McpPanel />
    case 'logs': return <LogViewer />
    default: return <PlaceholderPanel name="Agents" />
  }
}
