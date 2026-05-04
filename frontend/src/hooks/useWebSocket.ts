import { useState, useEffect, useRef, useCallback } from 'react';
import type { WsMessage } from '../types';

export type WsStatus = 'connecting' | 'connected' | 'disconnected' | 'error';

interface UseWebSocketOptions {
  enabled?: boolean;
}

export function useWebSocket(url: string | null, options: UseWebSocketOptions = {}) {
  const { enabled = true } = options;
  const [status, setStatus] = useState<WsStatus>('disconnected');
  const [messages, setMessages] = useState<WsMessage[]>([]);
  const wsRef = useRef<WebSocket | null>(null);
  const reconnectTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const reconnectAttempts = useRef(0);
  const pendingMessages = useRef<string[]>([]);

  const getReconnectDelay = useCallback(() => {
    const delay = Math.min(1000 * Math.pow(2, reconnectAttempts.current), 30000);
    reconnectAttempts.current += 1;
    return delay;
  }, []);

  const connect = useCallback(() => {
    if (!url || !enabled) return;

    setStatus('connecting');
    const ws = new WebSocket(url);
    wsRef.current = ws;

    ws.onopen = () => {
      setStatus('connected');
      reconnectAttempts.current = 0;
      // Flush pending messages
      while (pendingMessages.current.length > 0) {
        ws.send(pendingMessages.current.shift()!);
      }
    };

    ws.onmessage = (event) => {
      try {
        const msg = JSON.parse(event.data) as WsMessage;
        setMessages((prev) => [...prev, msg]);
      } catch {
        console.warn('Failed to parse WS message:', event.data);
      }
    };

    ws.onclose = () => {
      setStatus('disconnected');
      wsRef.current = null;
      if (enabled && url) {
        const delay = getReconnectDelay();
        reconnectTimerRef.current = setTimeout(connect, delay);
      }
    };

    ws.onerror = () => {
      setStatus('error');
    };
  }, [url, enabled, getReconnectDelay]);

  const disconnect = useCallback(() => {
    if (reconnectTimerRef.current) {
      clearTimeout(reconnectTimerRef.current);
      reconnectTimerRef.current = null;
    }
    if (wsRef.current) {
      wsRef.current.close();
      wsRef.current = null;
    }
    setStatus('disconnected');
  }, []);

  const send = useCallback(
    (message: object) => {
      const text = JSON.stringify(message);
      if (wsRef.current?.readyState === WebSocket.OPEN) {
        wsRef.current.send(text);
      } else {
        pendingMessages.current.push(text);
      }
    },
    [],
  );

  useEffect(() => {
    if (url && enabled) {
      connect();
    }
    return () => {
      disconnect();
    };
  }, [url, enabled, connect, disconnect]);

  // Reset messages when URL changes (new session)
  useEffect(() => {
    setMessages([]);
  }, [url]);

  return { status, send, messages, connect, disconnect };
}
