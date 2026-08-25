// frontend/src/components/layout/TabBar.tsx
import { useAtom } from 'jotai'
import { activeTabAtom } from '@/stores/ui'
import type { ActiveTab } from '@/types'
import { TabsList, TabsTrigger } from '@/components/ui/tabs'

const TABS: { id: ActiveTab; label: string }[] = [
  { id: 'tasks', label: 'Tasks' },
  { id: 'agents', label: 'Agents' },
  { id: 'tools', label: 'Tools' },
  { id: 'workspace', label: 'Workspace' },
  { id: 'skills', label: 'Skills' },
  { id: 'mcp', label: 'MCP' },
  { id: 'sandboxes', label: 'Sandboxes' },
  { id: 'logs', label: 'Logs' },
]

export function TabBar() {
  const [, setActive] = useAtom(activeTabAtom)

  return (
    <TabsList className="flex flex-nowrap bg-card border-b border-border rounded-none h-auto p-0 w-full justify-start overflow-x-auto flex-shrink-0">
      {TABS.map((tab) => (
        <TabsTrigger
          key={tab.id}
          value={tab.id}
          onClick={() => setActive(tab.id)}
          className="cursor-pointer px-2 sm:px-4 py-1 sm:py-1.5 text-[11px] sm:text-[13px] whitespace-nowrap flex-shrink-0 rounded-none border-b-2 border-transparent data-[state=active]:border-primary data-[state=active]:bg-background data-[state=active]:text-foreground data-[state=active]:shadow-none"
        >
          {tab.label}
        </TabsTrigger>
      ))}
    </TabsList>
  )
}
