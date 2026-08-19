// frontend/src/hooks/useImageAttachments.ts
// Shared image-attachment state hook: adds files (queue + compress into data
// URLs), removes chips, and exposes the current list. Used by CapabilityBar
// (Attach trigger) and InputArea (chips, paste/drop, submit).
import { useAtom } from 'jotai'
import { useCallback } from 'react'
import { compressImageFile, ImageError } from '@/lib/image'
import { imageAttachmentsAtom, queueImageAttachments } from '@/stores/attachments'

export function useImageAttachments() {
  const [images, setImages] = useAtom(imageAttachmentsAtom)

  const addFiles = useCallback(
    (files: File[]) => {
      const { next, selected } = queueImageAttachments(images, files)
      if (selected.length === 0) return
      setImages(next)
      selected.forEach(({ file, id }) => {
        void compressImageFile(file).then(
          (dataUrl) => {
            setImages((cur) => cur.map((a) => (a.id === id ? { ...a, dataUrl } : a)))
          },
          (err: unknown) => {
            const message = err instanceof ImageError ? err.message : 'Could not process the image'
            setImages((cur) => cur.map((a) => (a.id === id ? { ...a, error: message } : a)))
          },
        )
      })
    },
    [images, setImages],
  )

  const removeImage = useCallback(
    (id: string) => {
      setImages((prev) => prev.filter((a) => a.id !== id))
    },
    [setImages],
  )

  return { images, setImages, addFiles, removeImage }
}
