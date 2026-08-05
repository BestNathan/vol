// frontend/src/hooks/useAutoScroll.ts
import { useRef, useCallback, useEffect } from 'react'

export function useAutoScroll(deps: unknown[]) {
  const containerRef = useRef<HTMLDivElement>(null)
  const autoScrollRef = useRef(true)
  const programmaticRef = useRef(false)

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
    programmaticRef.current = true
    vp.scrollTop = vp.scrollHeight
    requestAnimationFrame(() => { programmaticRef.current = false })
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
    }
    vp.addEventListener('scroll', onScroll, { passive: true })
    return () => vp.removeEventListener('scroll', onScroll)
  }, [deps, getViewport]) // re-attach when content changes (viewport may be recreated)

  // Auto-scroll when deps change, but ONLY if the user hasn't scrolled up.
  useEffect(() => {
    if (autoScrollRef.current) {
      scrollToBottom()
    }
  }, deps)

  return { containerRef, scrollToBottom, autoScrollRef }
}
