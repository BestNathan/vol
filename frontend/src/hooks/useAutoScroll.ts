// frontend/src/hooks/useAutoScroll.ts
import { useRef, useCallback, useEffect } from 'react'

export function useAutoScroll(deps: unknown[]) {
  const containerRef = useRef<HTMLDivElement>(null)
  const autoScrollRef = useRef(true)
  const programmaticScrollRef = useRef(false)

  // Get the actual scrollable viewport (Radix ScrollArea wraps content in a viewport).
  const getViewport = useCallback((): HTMLElement | null => {
    const el = containerRef.current
    if (!el) return null
    // Radix ScrollArea viewport has data-radix-scroll-area-viewport attribute.
    const vp = el.querySelector('[data-radix-scroll-area-viewport]') as HTMLElement | null
    return vp ?? el
  }, [])

  const scrollToBottom = useCallback(() => {
    const vp = getViewport()
    if (!vp) return
    programmaticScrollRef.current = true
    vp.scrollTop = vp.scrollHeight
    requestAnimationFrame(() => { programmaticScrollRef.current = false })
  }, [getViewport])

  const handleScroll = useCallback(() => {
    if (programmaticScrollRef.current) return
    const vp = getViewport()
    if (!vp) return
    const threshold = 4
    const atBottom = vp.scrollHeight - vp.scrollTop - vp.clientHeight <= threshold
    autoScrollRef.current = atBottom
  }, [getViewport])

  // Scroll to bottom when deps change, but only if user hasn't scrolled up.
  useEffect(() => {
    if (autoScrollRef.current) {
      scrollToBottom()
    }
  }, deps)

  return { containerRef, handleScroll, scrollToBottom, autoScrollRef, isAtBottom: autoScrollRef.current }
}
