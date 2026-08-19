// Integration test: TabBar renders all 7 tabs inside a Tabs root and
// switching a tab updates the activeTab atom.
import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createStore, Provider } from 'jotai'
import { TabBar } from '@/components/layout/TabBar'
import { Tabs } from '@/components/ui/tabs'
import { activeTabAtom } from '@/stores/ui'

function renderTabBar(store: ReturnType<typeof createStore>) {
  return render(
    <Provider store={store}>
      {/* Mirrors App.tsx: TabBar lives inside a Tabs root controlled by activeTab. */}
      <Tabs value={store.get(activeTabAtom)} className="flex flex-col">
        <TabBar />
      </Tabs>
    </Provider>,
  )
}

describe('TabBar', () => {
  it('renders all 7 tabs', () => {
    const store = createStore()
    store.set(activeTabAtom, 'agents')
    renderTabBar(store)

    for (const label of ['Tasks', 'Agents', 'Tools', 'Workspace', 'Skills', 'MCP', 'Logs']) {
      expect(screen.getByRole('tab', { name: label })).toBeInTheDocument()
    }
  })

  it('marks the active tab as selected', () => {
    const store = createStore()
    store.set(activeTabAtom, 'agents')
    renderTabBar(store)

    expect(screen.getByRole('tab', { name: 'Agents' })).toHaveAttribute(
      'data-state',
      'active',
    )
  })

  it('updates activeTabAtom when another tab is clicked', async () => {
    const store = createStore()
    store.set(activeTabAtom, 'agents')
    const user = userEvent.setup()
    renderTabBar(store)

    await user.click(screen.getByRole('tab', { name: 'Tools' }))

    expect(store.get(activeTabAtom)).toBe('tools')
  })
})
