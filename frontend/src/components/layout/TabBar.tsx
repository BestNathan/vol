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
    <div className="flex flex-nowrap bg-[#252540] border-b border-[#333355] flex-shrink-0 overflow-x-auto">
      {TABS.map(tab => (
        <button
          key={tab.id}
          onClick={() => setActive(tab.id)}
          className={cn(
            'px-2 sm:px-4 py-1 sm:py-1.5 cursor-pointer text-[11px] sm:text-[13px] whitespace-nowrap flex-shrink-0 border-b-2',
            active === tab.id
              ? 'bg-[#1a1a2e] text-[#e0e0e0] border-[#80a0ff]'
              : 'bg-transparent text-[#888] border-transparent hover:text-[#ccc] hover:bg-[#2a2a44]'
          )}
        >
          {tab.label}
        </button>
      ))}
    </div>
  )
}
