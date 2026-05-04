import React from 'react';
import { Badge } from 'antd';
import type { WsStatus } from '../hooks/useWebSocket';

interface StatusBadgeProps {
  status: WsStatus;
}

const statusMap: Record<WsStatus, { color: string; text: string }> = {
  connecting: { color: 'orange', text: 'Connecting...' },
  connected: { color: 'green', text: 'Connected' },
  disconnected: { color: 'default', text: 'Disconnected' },
  error: { color: 'red', text: 'Connection Error' },
};

export const StatusBadge: React.FC<StatusBadgeProps> = ({ status }) => {
  const { color, text } = statusMap[status];
  return <Badge status={color as 'default' | 'error' | 'processing' | 'success' | 'warning'} text={text} />;
};
