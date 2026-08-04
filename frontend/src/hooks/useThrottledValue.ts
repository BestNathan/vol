// frontend/src/hooks/useThrottledValue.ts
import { useState, useEffect, useRef } from 'react'

export function useThrottledValue<T>(value: T, delayMs: number): T {
  const [throttled, setThrottled] = useState(value)
  const lastUpdate = useRef(Date.now())
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  useEffect(() => {
    const elapsed = Date.now() - lastUpdate.current
    if (elapsed >= delayMs) {
      lastUpdate.current = Date.now()
      setThrottled(value)
    } else {
      if (timerRef.current) clearTimeout(timerRef.current)
      timerRef.current = setTimeout(() => {
        lastUpdate.current = Date.now()
        setThrottled(value)
      }, delayMs - elapsed)
    }
    return () => { if (timerRef.current) clearTimeout(timerRef.current) }
  }, [value, delayMs])

  return throttled
}
