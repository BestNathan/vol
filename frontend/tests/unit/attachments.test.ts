// frontend/tests/unit/attachments.test.ts
import { describe, it, expect } from 'vitest'
import { queueImageAttachments, type ImageAttachment } from '@/stores/attachments'
import { MAX_IMAGES_PER_MESSAGE } from '@/lib/image'

function fakeFile(type: string, name = 'a.png'): File {
  return { type, name } as File
}

const pending = (id: string): ImageAttachment => ({ id, dataUrl: null, error: null })

describe('queueImageAttachments', () => {
  it('returns a pending attachment per image file', () => {
    const files = [fakeFile('image/png', 'a.png'), fakeFile('image/jpeg', 'b.jpg')]

    const { next, selected } = queueImageAttachments([], files)

    expect(next).toHaveLength(2)
    for (const a of next) {
      expect(a.dataUrl).toBeNull()
      expect(a.error).toBeNull()
      expect(a.id).toBeTruthy()
    }
    expect(next[0].id).not.toBe(next[1].id)
    expect(selected.map((s) => s.id)).toEqual(next.map((a) => a.id))
    expect(selected.map((s) => s.file)).toEqual(files)
  })

  it('ignores non-image files', () => {
    const { next, selected } = queueImageAttachments([], [fakeFile('text/plain', 'a.txt')])

    expect(next).toEqual([])
    expect(selected).toEqual([])
  })

  it('caps the total at MAX_IMAGES_PER_MESSAGE', () => {
    const existing = Array.from({ length: MAX_IMAGES_PER_MESSAGE - 1 }, (_, i) =>
      pending(`old-${i}`),
    )
    const files = [1, 2, 3].map((n) => fakeFile('image/png', `${n}.png`))

    const { next, selected } = queueImageAttachments(existing, files)

    expect(next).toHaveLength(MAX_IMAGES_PER_MESSAGE)
    expect(selected).toHaveLength(1)
  })

  it('returns the current list unchanged when already at the cap', () => {
    const existing = Array.from({ length: MAX_IMAGES_PER_MESSAGE }, (_, i) =>
      pending(`old-${i}`),
    )

    const { next, selected } = queueImageAttachments(existing, [fakeFile('image/png')])

    expect(next).toBe(existing)
    expect(selected).toEqual([])
  })
})
