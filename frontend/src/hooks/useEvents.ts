import { useState, useEffect, useRef, useCallback } from 'react';
import type { ManagerEvent } from '../types';

const MAX_EVENTS = 100;

interface UseEventsOptions {
  enabled?: boolean;
}

export function useEvents(options: UseEventsOptions = {}) {
  const { enabled = true } = options;
  const [events, setEvents] = useState<ManagerEvent[]>([]);
  const [paused, setPaused] = useState(false);
  const [connected, setConnected] = useState(false);
  const esRef = useRef<EventSource | null>(null);

  const togglePause = useCallback(() => {
    setPaused((prev) => !prev);
  }, []);

  useEffect(() => {
    if (!enabled) return;

    const es = new EventSource('/api/v1/events');
    esRef.current = es;

    es.onopen = () => setConnected(true);

    es.onmessage = (event) => {
      if (paused) return;
      try {
        const evt = JSON.parse(event.data) as ManagerEvent;
        setEvents((prev) => {
          const next = [...prev, evt];
          if (next.length > MAX_EVENTS) {
            return next.slice(next.length - MAX_EVENTS);
          }
          return next;
        });
      } catch {
        console.warn('Failed to parse SSE event:', event.data);
      }
    };

    es.onerror = () => {
      setConnected(false);
      // EventSource auto-reconnects, but we track connection state
    };

    return () => {
      es.close();
      esRef.current = null;
    };
  }, [enabled, paused]);

  const clear = useCallback(() => {
    setEvents([]);
  }, []);

  return { events, connected, paused, togglePause, clear };
}
