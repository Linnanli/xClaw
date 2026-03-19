import React, { useState } from 'react';
import { Layout, Menu, Dropdown, Avatar, Space, Breadcrumb } from 'antd';
import {
  DashboardOutlined,
  UserOutlined,
  SafetyOutlined,
  AuditOutlined,
  LaptopOutlined,
  AppstoreOutlined,
  SettingOutlined,
  BarChartOutlined,
  LogoutOutlined,
  MenuFoldOutlined,
  MenuUnfoldOutlined,
} from '@ant-design/icons';
import { Outlet, useNavigate, useLocation } from 'react-router-dom';
import { useAuthStore } from '../../store/authStore';
import type { MenuProps } from 'antd';
import '../../styles/MainLayout.css';

const { Header, Sider, Content } = Layout;

export const MainLayout: React.FC = () => {
  const [collapsed, setCollapsed] = useState(false);
  const navigate = useNavigate();
  const location = useLocation();
  const { user, logout } = useAuthStore();

  // 菜单项配置
  const menuItems: MenuProps['items'] = [
    {
      key: '/dashboard',
      icon: <DashboardOutlined />,
      label: '仪表盘',
    },
    {
      key: '/users',
      icon: <UserOutlined />,
      label: '用户管理',
      children: [
        { key: '/users/list', label: '用户列表' },
        { key: '/users/roles', label: '角色管理' },
        { key: '/users/permissions', label: '权限管理' },
      ],
    },
    {
      key: '/security',
      icon: <SafetyOutlined />,
      label: '安全策略',
      children: [
        { key: '/security/dlp-rules', label: 'DLP 规则' },
        { key: '/security/sensitive-ops', label: '敏感操作' },
        { key: '/security/policy-versions', label: '策略版本' },
      ],
    },
    {
      key: '/audit',
      icon: <AuditOutlined />,
      label: '审计日志',
    },
    {
      key: '/clients',
      icon: <LaptopOutlined />,
      label: '客户端管理',
    },
    {
      key: '/extensions',
      icon: <AppstoreOutlined />,
      label: '扩展管理',
      children: [
        { key: '/extensions/skills', label: '技能管理' },
        { key: '/extensions/plugins', label: '插件管理' },
      ],
    },
    {
      key: '/settings',
      icon: <SettingOutlined />,
      label: '系统配置',
    },
    {
      key: '/reports',
      icon: <BarChartOutlined />,
      label: '统计报表',
    },
  ];

  // 用户菜单
  const userMenuItems: MenuProps['items'] = [
    {
      key: 'profile',
      label: '个人信息',
      icon: <UserOutlined />,
    },
    {
      key: 'settings',
      label: '设置',
      icon: <SettingOutlined />,
    },
    {
      type: 'divider',
    },
    {
      key: 'logout',
      label: '退出登录',
      icon: <LogoutOutlined />,
      danger: true,
    },
  ];

  const handleMenuClick = ({ key }: { key: string }) => {
    navigate(key);
  };

  const handleUserMenuClick: MenuProps['onClick'] = ({ key }) => {
    if (key === 'logout') {
      logout();
      navigate('/login');
    } else if (key === 'profile') {
      navigate('/profile');
    } else if (key === 'settings') {
      navigate('/settings');
    }
  };

  // 生成面包屑
  const getBreadcrumbs = () => {
    const pathSnippets = location.pathname.split('/').filter((i) => i);
    const breadcrumbItems = pathSnippets.map((_, index) => {
      const url = `/${pathSnippets.slice(0, index + 1).join('/')}`;
      return {
        title: getPageTitle(url),
      };
    });

    return [{ title: '首页' }, ...breadcrumbItems];
  };

  const getPageTitle = (path: string): string => {
    const titles: Record<string, string> = {
      '/dashboard': '仪表盘',
      '/users': '用户管理',
      '/users/list': '用户列表',
      '/users/roles': '角色管理',
      '/users/permissions': '权限管理',
      '/security': '安全策略',
      '/security/dlp-rules': 'DLP 规则',
      '/security/sensitive-ops': '敏感操作',
      '/security/policy-versions': '策略版本',
      '/audit': '审计日志',
      '/clients': '客户端管理',
      '/extensions': '扩展管理',
      '/extensions/skills': '技能管理',
      '/extensions/plugins': '插件管理',
      '/settings': '系统配置',
      '/reports': '统计报表',
    };

    return titles[path] || '未知页面';
  };

  return (
    <Layout className="main-layout">
      <Sider trigger={null} collapsible collapsed={collapsed} className="layout-sider">
        <div className="logo">
          <h1>{collapsed ? 'IC' : 'IronClaw'}</h1>
        </div>
        <Menu
          theme="dark"
          mode="inline"
          selectedKeys={[location.pathname]}
          items={menuItems}
          onClick={handleMenuClick}
        />
      </Sider>

      <Layout>
        <Header className="layout-header">
          <div className="header-left">
            {React.createElement(collapsed ? MenuUnfoldOutlined : MenuFoldOutlined, {
              className: 'trigger',
              onClick: () => setCollapsed(!collapsed),
            })}
          </div>

          <div className="header-right">
            <Dropdown menu={{ items: userMenuItems, onClick: handleUserMenuClick }} placement="bottomRight">
              <Space className="user-menu" style={{ cursor: 'pointer' }}>
                <Avatar icon={<UserOutlined />} />
                <span>{user?.username || '管理员'}</span>
              </Space>
            </Dropdown>
          </div>
        </Header>

        <Content className="layout-content">
          <Breadcrumb items={getBreadcrumbs()} className="breadcrumb" />
          <div className="content-wrapper">
            <Outlet />
          </div>
        </Content>
      </Layout>
    </Layout>
  );
};
