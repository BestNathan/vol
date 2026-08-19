// Integration test: StatusBar renders connection/session/run info from the
// store, reflects the running state, and toggles the debug panel.
import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createStore, Provider } from 'jotai'
import { StatusBar } from '@/components/layout/StatusBar'
import {
  connectionStateAtom,
  isRunningAtom,
  iterationAtom,
  runCountAtom,
  runElapsedAtom,
  sessionIdAtom,
  toolCallCountAtom,
} from '@/stores/connection'
import { debugPanelAtom } from '@/stores/dialogs'

function renderStatusBar(store: ReturnType<typeof createStore>) {
  return render(
    <Provider store={store}>
      <StatusBar />
    </Provider>,
  )
}

describe('StatusBar', () => {
  it('shows connection indicator and session/run counters while idle', () => {
    const store = createStore()
    store.set(connectionStateAtom, 'connected')
    store.set(sessionIdAtom, 'web-abc12345')
    store.set(runCountAtom, 3)
    store.set(iterationAtom, 2)
    store.set(toolCallCountAtom, 1)
    store.set(runElapsedAtom, 65_000)
    renderStatusBar(store)

    expect(screen.getByText('Connected')).toBeInTheDocument()
    expect(screen.getByText('Session: web-abc1')).toBeInTheDocument() // sliced to 8 chars
    expect(screen.getByText('Run: 3')).toBeInTheDocument()
    expect(screen.getByText('Iter: 2')).toBeInTheDocument()
    expect(screen.getByText('Tools: 1')).toBeInTheDocument()
    expect(screen.getByText('Time: 01:05')).toBeInTheDocument()
    expect(screen.getByText('Idle')).toBeInTheDocument()
  })

  it('shows the Running badge while a run is active', () => {
    const store = createStore()
    store.set(connectionStateAtom, 'connected')
    store.set(isRunningAtom, true)
    renderStatusBar(store)

    expect(screen.getByText('Running')).toBeInTheDocument()
  })

  it('toggles the debug panel from the bug button', async () => {
    const store = createStore()
    const user = userEvent.setup()
    renderStatusBar(store)

    await user.click(screen.getByRole('button', { name: 'Toggle debug panel' }))

    expect(store.get(debugPanelAtom).open).toBe(true)
  })
})
