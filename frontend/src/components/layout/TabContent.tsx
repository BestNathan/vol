// frontend/src/components/layout/TabContent.tsx
import { useAtomValue } from 'jotai'
import { activeTabAtom } from '@/stores/ui'
import { AgentsPanel } from '@/components/panels/AgentsPanel'
import { ToolsTab } from '@/components/panels/ToolsTab'

function PlaceholderPanel({ name }: { name: string }) {
  return (
    <div className="flex items-center justify-center h-full text-[#666] text-sm">
      {name} — coming soon
    </div>
  )
}

export function TabContent() {
  const active = useAtomValue(activeTabAtom)

  switch (active) {
    case 'tasks': return <PlaceholderPanel name="Tasks" />
    case 'agents': return <AgentsPanel />
    case 'tools': return <ToolsTab />
    case 'workspace': return <PlaceholderPanel name="Workspace" />
    case 'skills': return <PlaceholderPanel name="Skills" />
    case 'mcp': return <PlaceholderPanel name="MCP" />
    case 'logs': return <PlaceholderPanel name="Logs" />
    default: return <PlaceholderPanel name="Agents" />
  }
}
