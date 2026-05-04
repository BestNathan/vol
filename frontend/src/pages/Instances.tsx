import React, { useState, useEffect, useCallback } from 'react';
import {
  Table, Button, Modal, Form, Input, Select, Space,
  Popconfirm, message, Tag, Alert, Spin,
} from 'antd';
import { PlusOutlined, DeleteOutlined, MessageOutlined, ReloadOutlined } from '@ant-design/icons';
import { useNavigate } from 'react-router-dom';
import { fetchInstances, destroyInstance } from '../api/instances';
import { fetchAgentTypes } from '../api/agentTypes';
import type { AgentInstanceSummary, AgentTypeMeta } from '../types';

export const Instances: React.FC = () => {
  const navigate = useNavigate();
  const [instances, setInstances] = useState<AgentInstanceSummary[]>([]);
  const [agentTypes, setAgentTypes] = useState<AgentTypeMeta[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [createModalOpen, setCreateModalOpen] = useState(false);
  const [form] = Form.useForm();

  const loadInstances = useCallback(async () => {
    try {
      const data = await fetchInstances();
      setInstances(data);
      setError(null);
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Failed to load');
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    Promise.all([fetchAgentTypes(), loadInstances()])
      .then(([types]) => { if (!cancelled) setAgentTypes(types); })
      .finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [loadInstances]);

  const handleCreate = async (values: { agentType: string; sessionId: string; parentSessionId?: string }) => {
    // Creation: open chat with new session, backend creates instance on WS connect
    setCreateModalOpen(false);
    navigate(`/chat/${values.agentType}/${values.sessionId}`);
  };

  const handleDestroy = async (agentType: string, sessionId: string) => {
    try {
      await destroyInstance(agentType, sessionId);
      message.success('Instance destroyed');
      loadInstances();
    } catch {
      message.error('Failed to destroy instance');
    }
  };

  const columns = [
    { title: 'Agent Type', dataIndex: 'agent_type', key: 'agent_type' },
    { title: 'Session ID', dataIndex: 'session_id', key: 'session_id', ellipsis: true },
    { title: 'Parent Session', dataIndex: 'parent_session_id', key: 'parent_session_id', ellipsis: true, render: (v: string | null) => v || '-' },
    {
      title: 'Status', dataIndex: 'status', key: 'status', width: 100,
      render: (status: string) => <Tag color={status === 'Running' ? 'green' : 'default'}>{status}</Tag>,
    },
    { title: 'Connections', dataIndex: 'connection_count', key: 'connection_count', width: 120 },
    {
      title: 'Created', dataIndex: 'created_at', key: 'created_at', width: 200,
      render: (v: string) => new Date(v).toLocaleString(),
    },
    {
      title: 'Actions', key: 'actions', width: 200,
      render: (_: unknown, record: AgentInstanceSummary) => (
        <Space>
          <Button
            type="link" size="small" icon={<MessageOutlined />}
            onClick={() => navigate(`/chat/${record.agent_type}/${record.session_id}`)}
          >
            Chat
          </Button>
          <Popconfirm
            title="Destroy this instance?"
            onConfirm={() => handleDestroy(record.agent_type, record.session_id)}
            okText="Destroy"
            cancelText="Cancel"
          >
            <Button type="link" danger size="small" icon={<DeleteOutlined />}>
              Destroy
            </Button>
          </Popconfirm>
        </Space>
      ),
    },
  ];

  if (loading) return <Spin size="large" style={{ display: 'block', margin: '48px auto' }} />;

  return (
    <div>
      <div style={{ marginBottom: 16, display: 'flex', justifyContent: 'space-between' }}>
        <Space>
          <Button type="primary" icon={<PlusOutlined />} onClick={() => setCreateModalOpen(true)}>
            New Instance
          </Button>
          <Button icon={<ReloadOutlined />} onClick={loadInstances}>Refresh</Button>
        </Space>
      </div>
      {error && <Alert message="Failed to load instances" description={error} type="error" showIcon style={{ marginBottom: 16 }} />}
      <Table
        columns={columns}
        dataSource={instances}
        rowKey={(r) => `${r.agent_type}/${r.session_id}`}
        pagination={false}
      />
      <Modal
        title="Create New Instance"
        open={createModalOpen}
        onCancel={() => setCreateModalOpen(false)}
        footer={null}
      >
        <Form form={form} onFinish={handleCreate} layout="vertical">
          <Form.Item name="agentType" label="Agent Type" rules={[{ required: true }]}>
            <Select>
              {agentTypes.map((t) => (
                <Select.Option key={t.name} value={t.name}>{t.name} ({t.type})</Select.Option>
              ))}
            </Select>
          </Form.Item>
          <Form.Item name="sessionId" label="Session ID" rules={[{ required: true }]}>
            <Input placeholder="Enter session ID" />
          </Form.Item>
          <Form.Item name="parentSessionId" label="Parent Session ID (optional)">
            <Input placeholder="For forked sessions" />
          </Form.Item>
          <Form.Item>
            <Button type="primary" htmlType="submit">Create & Chat</Button>
          </Form.Item>
        </Form>
      </Modal>
    </div>
  );
};
