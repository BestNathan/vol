// frontend/src/hooks/useAutoScroll.ts
import { useRef, useCallback, useEffect } from 'react'

export function useAutoScroll(deps: unknown[]) {
  const containerRef = useRef<HTMLDivElement>(null)
  const autoScrollRef = useRef(true)
  const programmaticScrollRef = useRef(false)

  const scrollToBottom = useCallback(() => {
    const el = containerRef.current
    if (!el) return
    programmaticScrollRef.current = true
    el.scrollTop = el.scrollHeight
    // Reset after a frame
    requestAnimationFrame(() => { programmaticScrollRef.current = false })
  }, [])

  const handleScroll = useCallback(() => {
    if (programmaticScrollRef.current) return
    const el = containerRef.current
    if (!el) return
    const threshold = 2
    const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight <= threshold
    autoScrollRef.current = atBottom
  }, [])

  useEffect(() => {
    if (autoScrollRef.current) {
      scrollToBottom()
    }
  }, deps)

  return { containerRef, handleScroll, scrollToBottom, autoScrollRef }
}
