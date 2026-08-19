// frontend/src/components/shared/ImageGallery.tsx
// Message image attachments: clickable thumbnails that open a lightbox
// Dialog with the full-size image (prev/next cycling for multiple images).
// Shared by ConversationView (live runs) and SessionDetailOverlay (sessions).
import { useState } from 'react'
import { ChevronLeftIcon, ChevronRightIcon } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogTitle } from '@/components/ui/dialog'

export function ImageGallery({ images }: { images: string[] }) {
  const [index, setIndex] = useState<number | null>(null)

  const open = index !== null
  const prev = () => setIndex((i) => (i === null ? null : (i - 1 + images.length) % images.length))
  const next = () => setIndex((i) => (i === null ? null : (i + 1) % images.length))

  return (
    <>
      <div className="flex flex-wrap gap-2 mt-2">
        {images.map((src, i) => (
          <Button
            key={i}
            variant="ghost"
            size="sm"
            onClick={() => setIndex(i)}
            aria-label={`View image ${i + 1}`}
            className="p-0 h-auto w-auto cursor-pointer rounded-md border border-border overflow-hidden hover:opacity-85"
          >
            <img
              src={src}
              alt={`attachment ${i + 1}`}
              className="w-24 h-24 object-cover rounded-md"
            />
          </Button>
        ))}
      </div>
      <Dialog
        open={open}
        onOpenChange={(nextOpen) => {
          if (!nextOpen) setIndex(null)
        }}
      >
        <DialogContent
          overlayClassName="bg-black/80"
          className="max-w-[90vw] w-auto max-h-[85vh] p-0 gap-0 overflow-hidden"
        >
          <DialogTitle className="sr-only">Image preview</DialogTitle>
          {index !== null && (
            <>
              <img
                src={images[index]}
                alt="Image preview"
                className="max-h-[85vh] max-w-[90vw] object-contain"
              />
              {images.length > 1 && (
                <>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={prev}
                    aria-label="Previous image"
                    className="absolute left-2 top-1/2 -translate-y-1/2 cursor-pointer bg-background/60 hover:bg-background/80"
                  >
                    <ChevronLeftIcon data-icon="inline-start" />
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={next}
                    aria-label="Next image"
                    className="absolute right-2 top-1/2 -translate-y-1/2 cursor-pointer bg-background/60 hover:bg-background/80"
                  >
                    <ChevronRightIcon data-icon="inline-start" />
                  </Button>
                </>
              )}
            </>
          )}
        </DialogContent>
      </Dialog>
    </>
  )
}
