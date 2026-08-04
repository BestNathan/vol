// frontend/src/components/layout/TabContent.tsx
import { useAtomValue } from 'jotai'
import { activeTabAtom, viewingNodeDetailAtom } from '@/stores/ui'
import { AgentsPanel } from '@/components/panels/AgentsPanel'
import { ToolsTab } from '@/components/panels/ToolsTab'
import { McpPanel } from '@/components/panels/McpPanel'
import { SkillsPanel } from '@/components/panels/SkillsPanel'
import { TasksPanel } from '@/components/panels/TasksPanel'
import { FileContentView } from '@/components/panels/FileContentView'
import { LogViewer } from '@/components/panels/LogViewer'
import { NodesPanel } from '@/components/panels/NodesPanel'

function PlaceholderPanel({ name }: { name: string }) {
  return (
    <div className="flex items-center justify-center h-full text-[#666] text-sm">
      {name} — coming soon
    </div>
  )
}

export function TabContent() {
  const active = useAtomValue(activeTabAtom)
  const viewingNodeDetail = useAtomValue(viewingNodeDetailAtom)

  // Node detail (opened by clicking a node name in the Nodes dropdown)
  // replaces the active tab's content while viewingNodeDetailAtom is set;
  // NodeDetailPanel's "← Back" clears it and restores the active tab.
  if (viewingNodeDetail) {
    return <NodesPanel />
  }

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
