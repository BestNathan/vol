import React, { useState, useEffect } from 'react';
import { Row, Col, Card, Statistic, Spin, Alert, List } from 'antd';
import {
  AppstoreOutlined,
  ClusterOutlined,
  CheckCircleOutlined,
} from '@ant-design/icons';
import { fetchAgentTypes } from '../api/agentTypes';
import { fetchInstances } from '../api/instances';
import { checkHealth } from '../api/health';
import type { AgentTypeMeta, AgentInstanceSummary, ManagerEvent } from '../types';

export const Dashboard: React.FC = () => {
  const [agentTypes, setAgentTypes] = useState<AgentTypeMeta[]>([]);
  const [instances, setInstances] = useState<AgentInstanceSummary[]>([]);
  const [healthy, setHealthy] = useState<boolean | null>(null);
  const [recentEvents, setRecentEvents] = useState<ManagerEvent[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    async function loadData() {
      try {
        setLoading(true);
        const [types, insts, ok] = await Promise.all([
          fetchAgentTypes(),
          fetchInstances(),
          checkHealth(),
        ]);
        if (!cancelled) {
          setAgentTypes(types);
          setInstances(insts);
          setHealthy(ok);
        }
      } catch (e: unknown) {
        if (!cancelled) {
          setError(e instanceof Error ? e.message : 'Failed to load data');
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    }
    loadData();
    return () => { cancelled = true; };
  }, []);

  // Fetch recent events via SSE
  useEffect(() => {
    const es = new EventSource('/api/v1/events');
    const events: ManagerEvent[] = [];
    es.onmessage = (event) => {
      try {
        const evt = JSON.parse(event.data) as ManagerEvent;
        events.push(evt);
        if (events.length > 5) events.shift();
        setRecentEvents([...events]);
      } catch { /* ignore */ }
    };
    return () => es.close();
  }, []);

  if (loading) return <Spin size="large" style={{ display: 'block', margin: '48px auto' }} />;
  if (error) return <Alert message="Failed to load dashboard data" description={error} type="error" showIcon />;

  return (
    <div>
      <Row gutter={[16, 16]}>
        <Col span={8}>
          <Card>
            <Statistic
              title="Agent Types"
              value={agentTypes.length}
              prefix={<AppstoreOutlined />}
            />
          </Card>
        </Col>
        <Col span={8}>
          <Card>
            <Statistic
              title="Running Instances"
              value={instances.length}
              prefix={<ClusterOutlined />}
            />
          </Card>
        </Col>
        <Col span={8}>
          <Card>
            <Statistic
              title="Health Status"
              value={healthy ? 'OK' : 'Down'}
              prefix={healthy ? <CheckCircleOutlined style={{ color: '#52c41a' }} /> : <CheckCircleOutlined style={{ color: '#ff4d4f' }} />}
            />
          </Card>
        </Col>
      </Row>
      <Card title="Recent Events" style={{ marginTop: 16 }}>
        {recentEvents.length === 0 ? (
          <div style={{ color: '#999' }}>No events received yet</div>
        ) : (
          <List
            size="small"
            dataSource={recentEvents}
            renderItem={(evt) => (
              <List.Item>
                <List.Item.Meta
                  title={evt.event_type}
                  description={new Date(evt.timestamp).toLocaleTimeString()}
                />
                <span style={{ fontSize: 12, color: '#666' }}>
                  {JSON.stringify(evt.payload).slice(0, 80)}...
                </span>
              </List.Item>
            )}
          />
        )}
      </Card>
    </div>
  );
};
