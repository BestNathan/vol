import React from 'react';
import { Table, Button, Space, Tag, Badge } from 'antd';
import { CaretRightOutlined, PauseOutlined, ClearOutlined } from '@ant-design/icons';
import { useEvents } from '../hooks/useEvents';

const columns = [
  {
    title: 'Timestamp', dataIndex: 'timestamp', key: 'timestamp', width: 180,
    render: (v: string) => new Date(v).toLocaleString(),
  },
  {
    title: 'Event Type', dataIndex: 'event_type', key: 'event_type', width: 200,
    render: (type: string) => <Tag color="blue">{type}</Tag>,
  },
  {
    title: 'Details', dataIndex: 'payload', key: 'payload',
    render: (payload: Record<string, unknown>) => (
      <pre style={{ margin: 0, fontSize: 12, maxHeight: 60, overflow: 'auto' }}>
        {JSON.stringify(payload, null, 2)}
      </pre>
    ),
  },
];

export const Events: React.FC = () => {
  const { events, connected, paused, togglePause, clear } = useEvents();

  return (
    <div>
      <div style={{ marginBottom: 16, display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <Space>
          <Badge status={connected ? 'success' : 'error'} text={connected ? 'Stream Connected' : 'Stream Disconnected'} />
          <span style={{ color: '#999' }}>{events.length} events</span>
        </Space>
        <Space>
          <Button icon={paused ? <CaretRightOutlined /> : <PauseOutlined />} onClick={togglePause}>
            {paused ? 'Resume' : 'Pause'}
          </Button>
          <Button icon={<ClearOutlined />} onClick={clear}>Clear</Button>
        </Space>
      </div>
      <Table
        columns={columns}
        dataSource={events}
        rowKey={(_, i) => String(i)}
        pagination={false}
        size="small"
        scroll={{ y: 500 }}
      />
    </div>
  );
};
