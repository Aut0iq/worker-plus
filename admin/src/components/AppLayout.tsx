import { Layout, Menu, Space, Typography } from 'antd';
import {
  DashboardOutlined,
  FileTextOutlined,
  GroupOutlined,
  LogoutOutlined,
  ScheduleOutlined,
  ShoppingOutlined,
  TeamOutlined,
  UnorderedListOutlined,
} from '@ant-design/icons';
import { Outlet, useLocation, useNavigate } from 'react-router-dom';
import { useAuth } from '../auth';
import NotifyBell from './NotifyBell';

const items = [
  { key: '/', icon: <DashboardOutlined />, label: 'Главная' },
  { key: '/users', icon: <TeamOutlined />, label: 'Пользователи' },
  { key: '/groups', icon: <GroupOutlined />, label: 'Группы' },
  { key: '/sessions', icon: <ScheduleOutlined />, label: 'ОПТ' },
  { key: '/shop', icon: <ShoppingOutlined />, label: 'Магазин' },
  { key: '/summaries', icon: <FileTextOutlined />, label: 'Сводки' },
  { key: '/logs', icon: <UnorderedListOutlined />, label: 'Журналы' },
];

export default function AppLayout() {
  const nav = useNavigate();
  const loc = useLocation();
  const { user, logout } = useAuth();
  return (
    <Layout style={{ minHeight: '100vh' }}>
      <Layout.Sider width={210} theme="dark">
        <div style={{ color: '#fff', padding: 16, fontWeight: 600, fontSize: 18 }}>Work+</div>
        <Menu
          theme="dark"
          mode="inline"
          selectedKeys={[loc.pathname === '/' ? '/' : `/${loc.pathname.split('/')[1]}`]}
          items={items}
          onClick={({ key }) => nav(key)}
        />
      </Layout.Sider>
      <Layout>
        <Layout.Header
          style={{
            background: '#fff',
            display: 'flex',
            justifyContent: 'flex-end',
            alignItems: 'center',
            gap: 16,
            paddingRight: 24,
          }}
        >
          <NotifyBell />
          <Typography.Text>{user?.full_name}</Typography.Text>
          <a onClick={logout}>
            <Space>
              <LogoutOutlined /> Выход
            </Space>
          </a>
        </Layout.Header>
        <Layout.Content style={{ padding: 24, overflow: 'auto' }}>
          <Outlet />
        </Layout.Content>
      </Layout>
    </Layout>
  );
}
