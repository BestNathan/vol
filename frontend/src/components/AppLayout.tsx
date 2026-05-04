import React, { useState, useEffect } from 'react';
import { Layout, Menu, Badge } from 'antd';
import {
  DashboardOutlined,
  AppstoreOutlined,
  ClusterOutlined,
  NotificationOutlined,
} from '@ant-design/icons';
import { useNavigate, useLocation } from 'react-router-dom';
import { checkHealth } from '../api/health';

const { Sider, Header, Content } = Layout;

const menuItems = [
  { key: '/', icon: <DashboardOutlined />, label: 'Dashboard' },
  { key: '/agent-types', icon: <AppstoreOutlined />, label: 'Agent Types' },
  { key: '/instances', icon: <ClusterOutlined />, label: 'Instances' },
  { key: '/events', icon: <NotificationOutlined />, label: 'Events' },
];

interface AppLayoutProps {
  children: React.ReactNode;
}

export const AppLayout: React.FC<AppLayoutProps> = ({ children }) => {
  const navigate = useNavigate();
  const location = useLocation();
  const [healthy, setHealthy] = useState<boolean | null>(null);

  useEffect(() => {
    checkHealth().then(setHealthy);
    const timer = setInterval(() => checkHealth().then(setHealthy), 30000);
    return () => clearInterval(timer);
  }, []);

  return (
    <Layout style={{ minHeight: '100vh' }}>
      <Sider theme="light" width={220}>
        <div style={{ padding: '16px', textAlign: 'center', fontSize: '16px', fontWeight: 'bold' }}>
          Vol Agent Manager
        </div>
        <Menu
          mode="inline"
          selectedKeys={[location.pathname.split('/chat')[0] || '/']}
          items={menuItems}
          onClick={({ key }) => navigate(key)}
        />
      </Sider>
      <Layout>
        <Header style={{ background: '#fff', padding: '0 24px', display: 'flex', alignItems: 'center' }}>
          <span>Agent Manager Console</span>
          <div style={{ marginLeft: 'auto' }}>
            {healthy === null ? null : healthy ? (
              <Badge status="success" text="Healthy" />
            ) : (
              <Badge status="error" text="Unavailable" />
            )}
          </div>
        </Header>
        <Content style={{ margin: '24px 16px', padding: 24, background: '#fff', minHeight: 280 }}>
          {children}
        </Content>
      </Layout>
    </Layout>
  );
};
