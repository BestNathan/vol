// Integration test: ImageGallery renders message attachments as clickable
// thumbnails and opens a lightbox Dialog with the full-size image.
import { describe, it, expect } from 'vitest'
import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { ImageGallery } from '@/components/shared/ImageGallery'

const IMG_A = 'data:image/png;base64,AAAA'
const IMG_B = 'data:image/png;base64,BBBB'

describe('ImageGallery', () => {
  it('renders a clickable thumbnail per image', () => {
    render(<ImageGallery images={[IMG_A, IMG_B]} />)

    expect(screen.getByRole('button', { name: 'View image 1' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'View image 2' })).toBeInTheDocument()
    expect(screen.getByAltText('attachment 1')).toHaveAttribute('src', IMG_A)
    expect(screen.getByAltText('attachment 2')).toHaveAttribute('src', IMG_B)
  })

  it('opens a lightbox dialog showing the clicked image', async () => {
    const user = userEvent.setup()
    render(<ImageGallery images={[IMG_A, IMG_B]} />)

    await user.click(screen.getByRole('button', { name: 'View image 2' }))

    const dialog = await screen.findByRole('dialog')
    expect(dialog).toHaveAttribute('data-state', 'open')
    expect(within(dialog).getByAltText('Image preview')).toHaveAttribute('src', IMG_B)
  })

  it('cycles through images with the next/previous buttons', async () => {
    const user = userEvent.setup()
    render(<ImageGallery images={[IMG_A, IMG_B]} />)

    await user.click(screen.getByRole('button', { name: 'View image 1' }))
    const dialog = await screen.findByRole('dialog')
    expect(within(dialog).getByAltText('Image preview')).toHaveAttribute('src', IMG_A)

    await user.click(within(dialog).getByRole('button', { name: 'Next image' }))
    expect(within(dialog).getByAltText('Image preview')).toHaveAttribute('src', IMG_B)

    // Next wraps around to the first image.
    await user.click(within(dialog).getByRole('button', { name: 'Next image' }))
    expect(within(dialog).getByAltText('Image preview')).toHaveAttribute('src', IMG_A)

    await user.click(within(dialog).getByRole('button', { name: 'Previous image' }))
    expect(within(dialog).getByAltText('Image preview')).toHaveAttribute('src', IMG_B)
  })

  it('does not show navigation buttons for a single image', async () => {
    const user = userEvent.setup()
    render(<ImageGallery images={[IMG_A]} />)

    await user.click(screen.getByRole('button', { name: 'View image 1' }))
    const dialog = await screen.findByRole('dialog')

    expect(within(dialog).queryByRole('button', { name: 'Next image' })).not.toBeInTheDocument()
    expect(within(dialog).queryByRole('button', { name: 'Previous image' })).not.toBeInTheDocument()
  })

  it('closes the lightbox via the close button', async () => {
    const user = userEvent.setup()
    render(<ImageGallery images={[IMG_A]} />)

    await user.click(screen.getByRole('button', { name: 'View image 1' }))
    const dialog = await screen.findByRole('dialog')

    await user.click(within(dialog).getByRole('button', { name: 'Close' }))
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
  })
})
