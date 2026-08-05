// frontend/src/hooks/useAutoScroll.ts
import { useRef, useState, useCallback, useEffect } from 'react'

export function useAutoScroll(deps: unknown[]) {
  const containerRef = useRef<HTMLDivElement>(null)
  const autoScrollRef = useRef(true)
  const programmaticRef = useRef(false)
  const [isAtBottom, setIsAtBottom] = useState(true) // for re-renders

  // Radix ScrollArea puts the real scrollable element inside as viewport.
  // We need to attach our scroll listener to THAT element.
  const getViewport = useCallback((): HTMLElement | null => {
    const el = containerRef.current
    if (!el) return null
    return (el.querySelector('[data-radix-scroll-area-viewport]') as HTMLElement | null) ?? el
  }, [])

  const scrollToBottom = useCallback(() => {
    const vp = getViewport()
    if (!vp) return
    // Re-enable auto-scroll so future messages keep following.
    autoScrollRef.current = true
    programmaticRef.current = true
    // Force reflow then scroll — sometimes Radix viewport needs a frame.
    requestAnimationFrame(() => {
      if (vp) vp.scrollTop = vp.scrollHeight
      requestAnimationFrame(() => {
        if (vp) vp.scrollTop = vp.scrollHeight
        programmaticRef.current = false
      })
    })
  }, [getViewport])

  // Attach scroll listener directly to the viewport element.
  useEffect(() => {
    const vp = getViewport()
    if (!vp) return
    const onScroll = () => {
      if (programmaticRef.current) return
      const threshold = 4
      const atBottom = vp.scrollHeight - vp.scrollTop - vp.clientHeight <= threshold
      autoScrollRef.current = atBottom
      setIsAtBottom(atBottom)
    }
    vp.addEventListener('scroll', onScroll, { passive: true })
    return () => vp.removeEventListener('scroll', onScroll)
  }, [deps, getViewport]) // re-attach when content changes (viewport may be recreated)

  const doScrollToBottom = useCallback(() => {
    autoScrollRef.current = true
    setIsAtBottom(true)
    scrollToBottom()
  }, [scrollToBottom])

  // Auto-scroll when deps change, but ONLY if the user hasn't scrolled up.
  useEffect(() => {
    if (autoScrollRef.current) {
      scrollToBottom()
    }
  }, deps)

  return { containerRef, scrollToBottom: doScrollToBottom, isAtBottom }
}
