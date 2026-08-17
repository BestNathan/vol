// frontend/tests/unit/image.test.ts
import { describe, it, expect } from 'vitest'
import {
  ImageError,
  KEEP_AS_IS_THRESHOLD,
  MAX_LONG_EDGE,
  MAX_ORIGINAL_BYTES,
  needsReencode,
  validateImageFile,
} from '@/lib/image'

function fakeFile(size: number, type: string): File {
  return { size, type } as File
}

describe('validateImageFile', () => {
  it('accepts supported image types under the size cap', () => {
    expect(() => validateImageFile(fakeFile(1024, 'image/png'))).not.toThrow()
    expect(() => validateImageFile(fakeFile(1024, 'image/jpeg'))).not.toThrow()
    expect(() => validateImageFile(fakeFile(1024, 'image/webp'))).not.toThrow()
    expect(() => validateImageFile(fakeFile(1024, 'image/gif'))).not.toThrow()
  })

  it('rejects files over 10MB', () => {
    expect(() => validateImageFile(fakeFile(MAX_ORIGINAL_BYTES + 1, 'image/png'))).toThrowError(
      expect.objectContaining({ kind: 'TooLarge' }) as unknown as Error,
    )
  })

  it('rejects non-image types', () => {
    expect(() => validateImageFile(fakeFile(1024, 'text/plain'))).toThrowError(
      expect.objectContaining({ kind: 'UnsupportedType' }) as unknown as Error,
    )
  })
})

describe('needsReencode', () => {
  it('keeps small images under the long-edge cap as-is', () => {
    expect(needsReencode(KEEP_AS_IS_THRESHOLD, 1000, 1000)).toBe(false)
  })

  it('re-encodes images over the long-edge cap', () => {
    expect(needsReencode(KEEP_AS_IS_THRESHOLD - 1, MAX_LONG_EDGE + 1, 100)).toBe(true)
  })

  it('re-encodes images over the byte threshold', () => {
    expect(needsReencode(KEEP_AS_IS_THRESHOLD + 1, 100, 100)).toBe(true)
  })
})
