// Integration test: CapabilityBar fetches capabilities for the selected
// agent via the panel client and shows summary counts; the ✎ button opens
// the drawer (disabled without a selected agent).
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createStore, Provider } from 'jotai'
import { CapabilityBar } from '@/components/inputs/CapabilityBar'
import { selectedAgentIdAtom } from '@/stores/agents'
import { drawerOpenAtom } from '@/stores/capability'
import { approvalPendingAtom } from '@/stores/dialogs'
import { isRunningAtom } from '@/stores/connection'
import { getPanelClient } from '@/lib/panel-client'

vi.mock('@/lib/panel-client', () => ({
  getPanelClient: vi.fn(),
  getControlClient: vi.fn(),
}))

const mockCall = vi.fn()

function renderCapabilityBar(store: ReturnType<typeof createStore>) {
  return render(
    <Provider store={store}>
      <CapabilityBar />
    </Provider>,
  )
}

describe('CapabilityBar', () => {
  beforeEach(() => {
    mockCall.mockReset()
    mockCall.mockResolvedValue({
      effective_tools: ['bash', 'read_file'],
      effective_skills: ['explore'],
      effective_mcp_servers: ['filesystem'],
      available_tools: [],
      available_skills: [],
      available_mcp_servers: [],
      base_tools: [],
      base_skills: [],
      base_mcp_servers: [],
    })
    vi.mocked(getPanelClient).mockReturnValue({ call: mockCall } as never)
  })

  it('loads and shows capability counts for the selected agent', async () => {
    const store = createStore()
    store.set(selectedAgentIdAtom, 'agent-1')
    renderCapabilityBar(store)

    expect(await screen.findByText(/2 tools · 1 skills · 1 MCPs/)).toBeInTheDocument()
    expect(mockCall).toHaveBeenCalledWith('agent.get_capabilities', {
      agent_id: 'agent-1',
      session_id: 'web-session',
    })
  })

  it('opens the drawer from the edit button', async () => {
    const store = createStore()
    store.set(selectedAgentIdAtom, 'agent-1')
    const user = userEvent.setup()
    renderCapabilityBar(store)

    const edit = await screen.findByRole('button', { name: 'Edit capabilities' })
    await user.click(edit)

    expect(store.get(drawerOpenAtom)).toBe(true)
  })

  it('disables the edit button without a selected agent', () => {
    const store = createStore()
    renderCapabilityBar(store)

    expect(screen.getByRole('button', { name: 'Edit capabilities' })).toBeDisabled()
  })

  it('shows an Attach images button next to the edit button', async () => {
    const store = createStore()
    store.set(selectedAgentIdAtom, 'agent-1')
    renderCapabilityBar(store)

    expect(await screen.findByRole('button', { name: 'Attach images' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Edit capabilities' })).toBeInTheDocument()
  })

  it('opens the file picker when Attach images is clicked', async () => {
    const store = createStore()
    store.set(selectedAgentIdAtom, 'agent-1')
    const user = userEvent.setup()
    renderCapabilityBar(store)

    const clickSpy = vi.spyOn(HTMLInputElement.prototype, 'click').mockImplementation(() => {})
    await user.click(await screen.findByRole('button', { name: 'Attach images' }))
    expect(clickSpy).toHaveBeenCalledTimes(1)
    clickSpy.mockRestore()
  })

  it('disables Attach images while a run is active', async () => {
    const store = createStore()
    store.set(selectedAgentIdAtom, 'agent-1')
    store.set(isRunningAtom, true)
    renderCapabilityBar(store)

    expect(await screen.findByRole('button', { name: 'Attach images' })).toBeDisabled()
  })

  it('disables Attach images while a tool approval is pending', async () => {
    const store = createStore()
    store.set(selectedAgentIdAtom, 'agent-1')
    store.set(approvalPendingAtom, true)
    renderCapabilityBar(store)

    expect(await screen.findByRole('button', { name: 'Attach images' })).toBeDisabled()
  })
})
