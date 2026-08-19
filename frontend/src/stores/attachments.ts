// frontend/src/stores/attachments.ts
// Image attachments pending for the next agent.submit. Shared between
// CapabilityBar (the Attach trigger + file picker) and InputArea (chips,
// paste/drop, and submit payload), so the attachments survive across both.
import { atom } from 'jotai'
import { MAX_IMAGES_PER_MESSAGE } from '@/lib/image'

export interface ImageAttachment {
  id: string
  dataUrl: string | null // null while compressing
  error: string | null
}

export const imageAttachmentsAtom = atom<ImageAttachment[]>([])

let attachSeq = 0

/** Result of queuing files: the next attachment list plus the files chosen
 *  for compression (with their assigned ids). */
export interface QueuedImageAttachments {
  next: ImageAttachment[]
  selected: { file: File; id: string }[]
}

/**
 * Append the image files to the current attachment list as pending entries,
 * capping the total at MAX_IMAGES_PER_MESSAGE. Non-image files are ignored.
 * Pure decision logic (unit-tested); compression runs outside this function.
 */
export function queueImageAttachments(
  current: ImageAttachment[],
  files: File[],
): QueuedImageAttachments {
  const imageFiles = files.filter((f) => f.type.startsWith('image/'))
  const room = MAX_IMAGES_PER_MESSAGE - current.length
  if (imageFiles.length === 0 || room <= 0) {
    return { next: current, selected: [] }
  }
  const chosen = imageFiles.slice(0, room)
  const pending: ImageAttachment[] = chosen.map((f, i) => ({
    id: `attach-${attachSeq++}-${i}-${f.name}`,
    dataUrl: null,
    error: null,
  }))
  return {
    next: [...current, ...pending].slice(0, MAX_IMAGES_PER_MESSAGE),
    selected: chosen.map((f, i) => ({ file: f, id: pending[i].id })),
  }
}
