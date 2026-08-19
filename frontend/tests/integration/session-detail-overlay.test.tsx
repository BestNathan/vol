// Integration test: SessionDetailOverlay renders session entries fetched via
// session.entries. User messages with multipart image content must show the
// image thumbnails (not just the text) — regression test for the sessions tab
// dropping image attachments.
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createStore, Provider } from 'jotai'
import { SessionDetailOverlay } from '@/components/dialogs/SessionDetailOverlay'
import { getPanelClient } from '@/lib/panel-client'
import type { SessionListEntry } from '@/types'

vi.mock('@/lib/panel-client', () => ({
  getPanelClient: vi.fn(),
  getControlClient: vi.fn(),
}))

const mockCall = vi.fn()

const SESSION: SessionListEntry = { id: 'sess-1', entry_count: 1, created_at: 1 }

const IMAGE_URL = 'data:image/png;base64,QUJD'

// Real wire shape of a persisted user message with multipart content
// (data.message.message.message = vol_llm_core::Message).
const imageUserEntry = {
  id: 'id-1',
  session_id: 'sess-1',
  created_at: 1,
  type: 'message',
  data: {
    message: {
      message: {
        message: {
          role: 'user',
          content: [
            { type: 'text', text: 'look at this' },
            { type: 'image', image_url: { url: IMAGE_URL } },
          ],
        },
      },
    },
  },
}

function renderOverlay(store: ReturnType<typeof createStore>) {
  return render(
    <Provider store={store}>
      <SessionDetailOverlay session={SESSION} agentId="agent-1" open={true} onClose={vi.fn()} />
    </Provider>,
  )
}

describe('SessionDetailOverlay', () => {
  beforeEach(() => {
    mockCall.mockReset()
    mockCall.mockResolvedValue({ entries: [] })
    vi.mocked(getPanelClient).mockReturnValue({ call: mockCall } as never)
  })

  it('fetches session entries for the opened session', async () => {
    mockCall.mockResolvedValue({
      entries: [
        {
          id: 'id-0',
          session_id: 'sess-1',
          created_at: 0,
          type: 'message',
          data: {
            message: {
              message: {
                message: { role: 'user', content: 'plain hello' },
              },
            },
          },
        },
      ],
    })
    const store = createStore()
    renderOverlay(store)

    expect(await screen.findByText('plain hello')).toBeInTheDocument()
    expect(mockCall).toHaveBeenCalledWith('session.entries', {
      session_id: 'sess-1',
      agent_id: 'agent-1',
    })
  })

  it('renders image thumbnails for user messages with image parts', async () => {
    mockCall.mockResolvedValue({ entries: [imageUserEntry] })
    const store = createStore()
    renderOverlay(store)

    expect(await screen.findByText('look at this')).toBeInTheDocument()
    expect(screen.getByAltText('attachment 1')).toHaveAttribute('src', IMAGE_URL)
  })

  it('opens the lightbox when a session image thumbnail is clicked', async () => {
    mockCall.mockResolvedValue({ entries: [imageUserEntry] })
    const store = createStore()
    const user = userEvent.setup()
    renderOverlay(store)

    await user.click(await screen.findByRole('button', { name: 'View image 1' }))

    const dialog = await screen.findByRole('dialog')
    expect(within(dialog).getByAltText('Image preview')).toHaveAttribute('src', IMAGE_URL)
  })
})
