import React, { useState, useEffect } from 'react';
import { Table, Button, Empty, Spin, Alert } from 'antd';
import { MessageOutlined } from '@ant-design/icons';
import { useNavigate } from 'react-router-dom';
import { fetchAgentTypes } from '../api/agentTypes';
import type { AgentTypeMeta } from '../types';

const columns = [
  { title: 'Name', dataIndex: 'name', key: 'name' },
  { title: 'Type', dataIndex: 'type', key: 'type' },
  { title: 'Description', dataIndex: 'description', key: 'description' },
  { title: 'Scope', dataIndex: 'scope', key: 'scope', width: 100 },
  {
    title: 'Actions',
    key: 'actions',
    width: 140,
    render: (_: unknown, record: AgentTypeMeta) => {
      const navigate = useNavigate();
      return (
        <Button
          type="primary"
          icon={<MessageOutlined />}
          size="small"
          onClick={() => navigate(`/chat/${record.name}/new`)}
        >
          Start Chat
        </Button>
      );
    },
  },
];

export const AgentTypes: React.FC = () => {
  const [types, setTypes] = useState<AgentTypeMeta[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    fetchAgentTypes()
      .then((data) => { if (!cancelled) setTypes(data); })
      .catch((e) => { if (!cancelled) setError(e.message); })
      .finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, []);

  if (loading) return <Spin size="large" style={{ display: 'block', margin: '48px auto' }} />;
  if (error) return <Alert message="Failed to load agent types" description={error} type="error" showIcon />;
  if (types.length === 0) {
    return (
      <Empty
        description="No agent types discovered"
        image={Empty.PRESENTED_IMAGE_SIMPLE}
      >
        <p style={{ color: '#999' }}>
          Add .md agent definition files to <code>.agents/agents/</code> directory.
        </p>
      </Empty>
    );
  }

  return (
    <Table
      columns={columns}
      dataSource={types}
      rowKey="name"
      pagination={false}
    />
  );
};
