// Integration test: InputArea submit flow against a mocked panel client.
// Renders the real component with a real jotai store — no WS connection.
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createStore, Provider } from 'jotai'
import { InputArea } from '@/components/inputs/InputArea'
import { selectedAgentIdAtom } from '@/stores/agents'
import { conversationMapAtom } from '@/stores/conversation'
import { approvalPendingAtom } from '@/stores/dialogs'
import { isRunningAtom } from '@/stores/connection'
import { getPanelClient } from '@/lib/panel-client'

vi.mock('@/lib/panel-client', () => ({
  getPanelClient: vi.fn(),
  getControlClient: vi.fn(),
}))

const mockCall = vi.fn()

function renderInputArea(store: ReturnType<typeof createStore>) {
  return render(
    <Provider store={store}>
      <InputArea />
    </Provider>,
  )
}

describe('InputArea', () => {
  beforeEach(() => {
    mockCall.mockReset()
    mockCall.mockResolvedValue({ run_id: 'run-1' })
    vi.mocked(getPanelClient).mockReturnValue({ call: mockCall } as never)
  })

  it('submits text on Enter and appends an optimistic UserInput entry', async () => {
    const store = createStore()
    store.set(selectedAgentIdAtom, 'agent-1')
    const user = userEvent.setup()
    renderInputArea(store)

    const textarea = screen.getByPlaceholderText('Type a message to the agent...')
    await user.type(textarea, 'hello agent{Enter}')

    expect(mockCall).toHaveBeenCalledWith('agent.submit', {
      input: {
        parts: [{ type: 'text', text: 'hello agent' }],
        metadata: { session_id: 'web-session' },
      },
      target: 'agent-1',
    })
    expect(textarea).toHaveValue('')
    const conv = store.get(conversationMapAtom).get('agent-1')
    expect(conv?.entries).toContainEqual({ type: 'UserInput', text: 'hello agent' })
  })

  it('does not submit an empty message', async () => {
    const store = createStore()
    store.set(selectedAgentIdAtom, 'agent-1')
    const user = userEvent.setup()
    renderInputArea(store)

    const textarea = screen.getByPlaceholderText('Type a message to the agent...')
    await user.type(textarea, '   {Enter}')

    expect(mockCall).not.toHaveBeenCalled()
    expect(store.get(conversationMapAtom).has('agent-1')).toBe(false)
  })

  it('does not submit when no agent is selected', async () => {
    const store = createStore()
    const user = userEvent.setup()
    renderInputArea(store)

    const textarea = screen.getByPlaceholderText('Type a message to the agent...')
    await user.type(textarea, 'orphan message{Enter}')

    expect(mockCall).not.toHaveBeenCalled()
  })

  it('disables the textarea while a run is active', () => {
    const store = createStore()
    store.set(selectedAgentIdAtom, 'agent-1')
    store.set(isRunningAtom, true)
    renderInputArea(store)

    expect(screen.getByPlaceholderText('Type a message to the agent...')).toBeDisabled()
  })

  it('replaces the textarea with a banner while an approval is pending', () => {
    const store = createStore()
    store.set(selectedAgentIdAtom, 'agent-1')
    store.set(approvalPendingAtom, true)
    renderInputArea(store)

    expect(screen.getByText(/Tool approval pending/)).toBeInTheDocument()
    expect(
      screen.queryByPlaceholderText('Type a message to the agent...'),
    ).not.toBeInTheDocument()
  })
})
