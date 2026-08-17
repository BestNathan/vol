// frontend/src/lib/image.ts
// Client-side image validation + compression. Pure decision logic lives here
// (unit-tested); the canvas/file plumbing is exercised via manual testing.
export type ImageErrorKind = 'TooLarge' | 'UnsupportedType' | 'CompressionFailed'

export class ImageError extends Error {
  constructor(
    public kind: ImageErrorKind,
    message: string,
  ) {
    super(message)
    this.name = 'ImageError'
  }
}

export const MAX_ORIGINAL_BYTES = 10 * 1024 * 1024
export const MAX_IMAGES_PER_MESSAGE = 4
export const MAX_LONG_EDGE = 1568
export const JPEG_QUALITY = 0.85
export const KEEP_AS_IS_THRESHOLD = 300 * 1024

const SUPPORTED_TYPES = ['image/png', 'image/jpeg', 'image/webp', 'image/gif']

/** Throw ImageError unless the file is a supported image type under the size cap. */
export function validateImageFile(file: File): void {
  if (!SUPPORTED_TYPES.includes(file.type)) {
    throw new ImageError('UnsupportedType', `Unsupported image type: ${file.type || 'unknown'}`)
  }
  if (file.size > MAX_ORIGINAL_BYTES) {
    throw new ImageError('TooLarge', 'Image exceeds the 10MB limit')
  }
}

/** True when the image must be downscaled/re-encoded before sending. */
export function needsReencode(bytes: number, width: number, height: number): boolean {
  return bytes > KEEP_AS_IS_THRESHOLD || Math.max(width, height) > MAX_LONG_EDGE
}

/** Load the file's pixel dimensions without rendering it into the DOM. */
function loadImage(file: File): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const url = URL.createObjectURL(file)
    const img = new Image()
    img.onload = () => {
      URL.revokeObjectURL(url)
      resolve(img)
    }
    img.onerror = () => {
      URL.revokeObjectURL(url)
      reject(new ImageError('CompressionFailed', 'Could not decode the image'))
    }
    img.src = url
  })
}

function toDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(reader.result as string)
    reader.onerror = () => reject(new ImageError('CompressionFailed', 'Could not read the image'))
    reader.readAsDataURL(file)
  })
}

/**
 * Validate + compress an image file to a data URL.
 * Small images (<=300KB, <=1568px long edge) pass through unchanged (keeps
 * PNG transparency); larger ones are downscaled and re-encoded as JPEG.
 */
export async function compressImageFile(file: File): Promise<string> {
  validateImageFile(file)
  const img = await loadImage(file)
  if (!needsReencode(file.size, img.naturalWidth, img.naturalHeight)) {
    return toDataUrl(file)
  }
  const scale = MAX_LONG_EDGE / Math.max(img.naturalWidth, img.naturalHeight)
  const canvas = document.createElement('canvas')
  canvas.width = Math.max(1, Math.round(img.naturalWidth * scale))
  canvas.height = Math.max(1, Math.round(img.naturalHeight * scale))
  const ctx = canvas.getContext('2d')
  if (!ctx) throw new ImageError('CompressionFailed', 'Canvas 2D context unavailable')
  ctx.drawImage(img, 0, 0, canvas.width, canvas.height)
  return canvas.toDataURL('image/jpeg', JPEG_QUALITY)
}
