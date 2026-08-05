// frontend/src/components/layout/TabBar.tsx
import { useAtom } from 'jotai'
import { activeTabAtom } from '@/stores/ui'
import type { ActiveTab } from '@/types'
import { cn } from '@/lib/utils'

const TABS: { id: ActiveTab; label: string }[] = [
  { id: 'tasks', label: 'Tasks' },
  { id: 'agents', label: 'Agents' },
  { id: 'tools', label: 'Tools' },
  { id: 'workspace', label: 'Workspace' },
  { id: 'skills', label: 'Skills' },
  { id: 'mcp', label: 'MCP' },
  { id: 'logs', label: 'Logs' },
]

export function TabBar() {
  const [active, setActive] = useAtom(activeTabAtom)

  return (
    <div className="flex flex-nowrap bg-card border-b border-border flex-shrink-0 overflow-x-auto">
      {TABS.map(tab => (
        <button
          key={tab.id}
          onClick={() => setActive(tab.id)}
          className={cn(
            'px-2 sm:px-4 py-1 sm:py-1.5 cursor-pointer text-[11px] sm:text-[13px] whitespace-nowrap flex-shrink-0 border-b-2',
            active === tab.id
              ? 'bg-background text-foreground border-primary'
              : 'bg-transparent text-muted-foreground border-transparent hover:text-foreground/80 hover:bg-secondary'
          )}
        >
          {tab.label}
        </button>
      ))}
    </div>
  )
}
