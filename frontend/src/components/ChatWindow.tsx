import React, { useRef, useEffect } from 'react';
import { Input, Button, Space, Typography } from 'antd';
import { SendOutlined } from '@ant-design/icons';
import { useWebSocket } from '../hooks/useWebSocket';
import { StatusBadge } from './StatusBadge';
import type { WsMessage } from '../types';

const { TextArea } = Input;
const { Text } = Typography;

interface ChatWindowProps {
  agentType: string;
  sessionId: string;
}

function renderMessage(msg: WsMessage, index: number) {
  switch (msg.message_type) {
    case 'connected':
      return (
        <div key={index} style={{ textAlign: 'center', color: '#999', fontSize: 12, margin: '8px 0' }}>
          Connected to {msg.agent_type} (session: {msg.session_id})
        </div>
      );
    case 'agent_complete':
      return (
        <div key={index} style={{ display: 'flex', justifyContent: 'flex-start', marginBottom: 12 }}>
          <div style={{
            background: '#f0f0f0', borderRadius: 12, padding: '10px 16px',
            maxWidth: '70%', whiteSpace: 'pre-wrap', wordBreak: 'break-word',
          }}>
            {msg.content}
            <div style={{ fontSize: 11, color: '#999', marginTop: 4 }}>
              Completed in {msg.iterations} iterations
            </div>
          </div>
        </div>
      );
    case 'agent_error':
      return (
        <div key={index} style={{ textAlign: 'center', color: '#ff4d4f', margin: '8px 0' }}>
          Error: {msg.error}
        </div>
      );
    default:
      return null;
  }
}

export const ChatWindow: React.FC<ChatWindowProps> = ({ agentType, sessionId }) => {
  const [input, setInput] = React.useState('');
  const messagesEndRef = useRef<HTMLDivElement>(null);

  const wsUrl = `/ws/agents/${agentType}/session/${sessionId}`;
  const { status, send, messages } = useWebSocket(wsUrl);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  const handleSend = () => {
    if (!input.trim() || status !== 'connected') return;
    send({ content: input.trim() });
    setInput('');
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: 'calc(100vh - 200px)' }}>
      {/* Header */}
      <div style={{ padding: '12px 16px', borderBottom: '1px solid #f0f0f0', display: 'flex', alignItems: 'center' }}>
        <Text strong>{agentType}</Text>
        <Text type="secondary" style={{ marginLeft: 8, fontSize: 12 }}>{sessionId}</Text>
        <div style={{ marginLeft: 'auto' }}>
          <StatusBadge status={status} />
        </div>
      </div>

      {/* Messages */}
      <div style={{ flex: 1, overflowY: 'auto', padding: 16 }}>
        {messages.map((msg, i) => renderMessage(msg, i))}
        {messages.length === 0 && status === 'connecting' && (
          <div style={{ textAlign: 'center', color: '#999', marginTop: 48 }}>Connecting...</div>
        )}
        {messages.length === 0 && status === 'error' && (
          <div style={{ textAlign: 'center', color: '#ff4d4f', marginTop: 48 }}>
            Connection failed. Retrying automatically...
          </div>
        )}
        <div ref={messagesEndRef} />
      </div>

      {/* Input */}
      <div style={{ padding: '12px 16px', borderTop: '1px solid #f0f0f0' }}>
        <Space.Compact style={{ width: '100%' }}>
          <TextArea
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Type a message... (Enter to send)"
            autoSize={{ minRows: 1, maxRows: 4 }}
            disabled={status !== 'connected'}
          />
          <Button
            type="primary"
            icon={<SendOutlined />}
            onClick={handleSend}
            disabled={status !== 'connected' || !input.trim()}
          >
            Send
          </Button>
        </Space.Compact>
      </div>
    </div>
  );
};
