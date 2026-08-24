// frontend/src/components/shared/ThinkingBlock.tsx
// Thinking content block: renders markdown, shows max 3 lines (scrolled to bottom
// to show the most recent thinking), with gradient mask indicating truncation.
// Click opens a dialog showing the full thinking content.
import { useEffect, useRef, useState } from 'react'
import { Markdown } from '@/components/shared/Markdown'
import { Dialog, DialogContent, DialogTitle } from '@/components/ui/dialog'

interface ThinkingBlockProps {
  content: string
}

const MAX_HEIGHT = 72 // ~3 lines at default text size

export function ThinkingBlock({ content }: ThinkingBlockProps) {
  const [dialogOpen, setDialogOpen] = useState(false)
  const [isTruncated, setIsTruncated] = useState(false)
  const containerRef = useRef<HTMLDivElement>(null)
  const contentRef = useRef<HTMLDivElement>(null)

  // Check if content exceeds max height and scroll to bottom to show latest lines
  useEffect(() => {
    const container = containerRef.current
    const contentEl = contentRef.current
    if (!container || !contentEl) return

    const checkTruncation = () => {
      const truncated = contentEl.scrollHeight > MAX_HEIGHT
      setIsTruncated(truncated)
      if (truncated) {
        // Scroll to bottom to show the most recent thinking
        container.scrollTop = container.scrollHeight
      }
    }

    // Check on mount and when content changes
    checkTruncation()

    // Use ResizeObserver to detect when markdown rendering completes
    const observer = new ResizeObserver(checkTruncation)
    observer.observe(contentEl)

    return () => observer.disconnect()
  }, [content])

  return (
    <>
      <div
        ref={containerRef}
        className="relative cursor-pointer group"
        style={{ maxHeight: MAX_HEIGHT, overflow: 'hidden' }}
        onClick={() => setDialogOpen(true)}
      >
        <div ref={contentRef} className="text-muted-foreground italic text-sm">
          <Markdown content={content || 'Thinking...'} />
        </div>
        {/* Gradient mask at top when content is truncated */}
        {isTruncated && (
          <div className="absolute inset-x-0 top-0 h-8 bg-gradient-to-b from-background/80 to-transparent pointer-events-none" />
        )}
        {/* Hover hint */}
        {isTruncated && (
          <div className="absolute inset-x-0 bottom-0 bg-gradient-to-t from-background to-transparent py-1 px-2 opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none">
            <span className="text-xs text-muted-foreground">Click to view full thinking</span>
          </div>
        )}
      </div>

      <Dialog open={dialogOpen} onOpenChange={setDialogOpen}>
        <DialogContent className="sm:max-w-[700px] max-h-[70vh] flex flex-col">
          <DialogTitle className="text-sm font-semibold text-muted-foreground">
            Thinking
          </DialogTitle>
          <div className="flex-1 overflow-y-auto mt-2">
            <div className="text-muted-foreground italic text-sm leading-relaxed">
              <Markdown content={content} />
            </div>
          </div>
        </DialogContent>
      </Dialog>
    </>
  )
}
