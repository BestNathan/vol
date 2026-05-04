import React from 'react';
import { useParams } from 'react-router-dom';
import { ChatWindow } from '../components/ChatWindow';

export const Chat: React.FC = () => {
  const { agentType, sessionId } = useParams<{ agentType: string; sessionId: string }>();

  if (!agentType || !sessionId) {
    return <div style={{ padding: 24, color: '#999' }}>Invalid agent or session parameters</div>;
  }

  return <ChatWindow agentType={agentType} sessionId={sessionId} />;
};
