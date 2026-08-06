// frontend/src/components/layout/StatusBar.tsx
import { useAtomValue, useSetAtom } from 'jotai'
import { ConnectionIndicator } from '@/components/shared/ConnectionIndicator'
import { NodesDropdown } from '@/components/shared/NodesDropdown'
import { Button } from '@/components/ui/button'
import {
  sessionIdAtom, runCountAtom, iterationAtom, toolCallCountAtom,
  isRunningAtom, exitingAtom, unsafeModeAtom, runElapsedAtom,
} from '@/stores/connection'
import { debugPanelAtom } from '@/stores/dialogs'

function formatElapsed(ms: number): string {
  const secs = Math.floor(ms / 1000)
  return `${String(Math.floor(secs / 60)).padStart(2, '0')}:${String(secs % 60).padStart(2, '0')}`
}

export function StatusBar() {
  const sessionId = useAtomValue(sessionIdAtom)
  const runCount = useAtomValue(runCountAtom)
  const iteration = useAtomValue(iterationAtom)
  const toolCallCount = useAtomValue(toolCallCountAtom)
  const isRunning = useAtomValue(isRunningAtom)
  const elapsed = useAtomValue(runElapsedAtom)
  const exiting = useAtomValue(exitingAtom)
  const unsafeMode = useAtomValue(unsafeModeAtom)
  const setDebugPanel = useSetAtom(debugPanelAtom)

  const statusLabel = isRunning ? 'Running' : 'Idle'
  const statusCls = isRunning
    ? 'flex items-center justify-between px-3 py-1 bg-[#2d2d44] text-[12px] font-mono flex-shrink-0 text-yellow-400'
    : 'flex items-center justify-between px-3 py-1 bg-[#2d2d44] text-[12px] font-mono flex-shrink-0 text-emerald-400'

  return (
    <div className={statusCls}>
      <div className="flex items-center gap-1.5 overflow-hidden flex-nowrap sm:gap-1">
        <ConnectionIndicator />
        <NodesDropdown />
        <span className="text-muted-foreground text-[11px] hidden sm:inline">Session: {sessionId.slice(0, 8)}</span>
        <span className="text-muted-foreground text-[11px]">Run: {runCount}</span>
        <span className="text-muted-foreground text-[11px] hidden sm:inline">Iter: {iteration}</span>
        <span className="text-muted-foreground text-[11px]">Tools: {toolCallCount}</span>
        <span className="text-muted-foreground text-[11px] hidden sm:inline">Time: {formatElapsed(elapsed)}</span>
        {isRunning && <span className="px-1.5 py-0.5 rounded-[3px] text-[11px] font-bold bg-yellow-950/30 text-yellow-400">{statusLabel}</span>}
        {!isRunning && <span className="px-1.5 py-0.5 rounded-[3px] text-[11px] font-bold bg-emerald-950/30 text-emerald-400">{statusLabel}</span>}
        {unsafeMode && <span className="px-1.5 py-0.5 rounded-[3px] text-[11px] font-bold bg-red-950/30 text-destructive">!! UNSAFE</span>}
        {exiting && <span className="px-1.5 py-0.5 rounded-[3px] text-[11px] font-bold bg-red-950/30 text-destructive">QUITTING</span>}
      </div>
      <div className="flex items-center gap-1 text-[11px] text-muted-foreground">
        <span className="hidden sm:inline">UI: {__BUILD_TIME__}</span>
        <Button
          variant="ghost"
          size="icon"
          className="cursor-pointer hover:text-white text-[14px]"
          aria-label="Toggle debug panel"
          title="Debug panel"
          onClick={() => setDebugPanel((prev) => ({ ...prev, open: !prev.open }))}
        >
          🐛
        </Button>
      </div>
    </div>
  )
}
