// Integration test: ConversationView renders UserInput image attachments as
// clickable thumbnails that open the lightbox — the live-run counterpart of
// the session overlay behavior.
import { describe, it, expect } from 'vitest'
import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createStore, Provider } from 'jotai'
import { ConversationView } from '@/components/panels/ConversationView'
import { activeAgentIdAtom, conversationMapAtom } from '@/stores/conversation'

const IMAGE_URL = 'data:image/png;base64,QUJD'

function renderConversationView(store: ReturnType<typeof createStore>) {
  return render(
    <Provider store={store}>
      <ConversationView />
    </Provider>,
  )
}

describe('ConversationView', () => {
  it('renders image thumbnails for UserInput entries', () => {
    const store = createStore()
    store.set(activeAgentIdAtom, 'agent-1')
    store.set(
      conversationMapAtom,
      new Map([
        [
          'agent-1',
          {
            entries: [
              { type: 'UserInput' as const, text: 'look at this', images: [IMAGE_URL] },
            ],
            autoScroll: true,
          },
        ],
      ]),
    )
    renderConversationView(store)

    expect(screen.getByText('look at this')).toBeInTheDocument()
    expect(screen.getByAltText('attachment 1')).toHaveAttribute('src', IMAGE_URL)
  })

  it('opens the lightbox when a conversation image thumbnail is clicked', async () => {
    const store = createStore()
    store.set(activeAgentIdAtom, 'agent-1')
    store.set(
      conversationMapAtom,
      new Map([
        [
          'agent-1',
          {
            entries: [
              { type: 'UserInput' as const, text: 'look', images: [IMAGE_URL] },
            ],
            autoScroll: true,
          },
        ],
      ]),
    )
    const user = userEvent.setup()
    renderConversationView(store)

    await user.click(screen.getByRole('button', { name: 'View image 1' }))

    const dialog = await screen.findByRole('dialog')
    expect(within(dialog).getByAltText('Image preview')).toHaveAttribute('src', IMAGE_URL)
  })
})
