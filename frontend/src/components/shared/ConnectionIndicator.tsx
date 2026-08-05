// frontend/src/components/shared/ConnectionIndicator.tsx
import { useAtomValue } from 'jotai'
import { connectionStateAtom, wsLastErrorAtom } from '@/stores/connection'

export function ConnectionIndicator() {
  const state = useAtomValue(connectionStateAtom)
  const error = useAtomValue(wsLastErrorAtom)

  const dotColor = state === 'connected' ? '#40c040' :
    state === 'connecting' ? '#f0c040' : '#c04040'

  const label = state === 'connected' ? 'Connected' :
    state === 'connecting' ? 'Connecting...' :
    error ? `Error: ${error}` : 'No connection'

  return (
    <span className="flex items-center gap-1 mr-1">
      <span
        className="w-2 h-2 rounded-full inline-block flex-shrink-0"
        style={{ backgroundColor: dotColor, boxShadow: `0 0 4px ${dotColor}` }}
      />
      <span className="text-[11px] text-muted-foreground hidden sm:inline">{label}</span>
    </span>
  )
}
