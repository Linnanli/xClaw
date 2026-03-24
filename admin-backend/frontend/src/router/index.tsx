import React from 'react';
import { createBrowserRouter, Navigate } from 'react-router-dom';
import { Login } from '../pages/Login';
import { Dashboard } from '../pages/Dashboard';
import { UserList } from '../pages/Users/UserList';
import { RoleList } from '../pages/Users/RoleList';
import { PermissionList } from '../pages/Users/PermissionList';
import { DlpRuleList } from '../pages/Security/DlpRuleList';
import { DlpDictionaryList } from '../pages/Security/DlpDictionaryList';
import { SensitiveOpList } from '../pages/Security/SensitiveOpList';
import { PolicyVersionList } from '../pages/Security/PolicyVersionList';
import { ClientList } from '../pages/ClientList';
import { Reports } from '../pages/Reports';
import { AuditLog } from '../pages/AuditLog';
import { SkillList } from '../pages/Extensions/SkillList';
import { PluginList } from '../pages/Extensions/PluginList';
import { Settings } from '../pages/Settings';
import { ClientConfig } from '../pages/ClientConfig';
import { ModelConfigs } from '../pages/ModelConfigs';
import { MainLayout } from '../components/Layout/MainLayout';
import { DepartmentList } from '../pages/Departments/DepartmentList';
import { useAuthStore } from '../store/authStore';

// 路由守卫：保护需要认证的路由
const ProtectedRoute: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const { isAuthenticated } = useAuthStore();

  if (!isAuthenticated) {
    return <Navigate to="/login" replace />;
  }

  return <>{children}</>;
};

// 公开路由：已登录用户不能访问
const PublicRoute: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const { isAuthenticated } = useAuthStore();

  if (isAuthenticated) {
    return <Navigate to="/dashboard" replace />;
  }

  return <>{children}</>;
};

export const router = createBrowserRouter([
  {
    path: '/login',
    element: (
      <PublicRoute>
        <Login />
      </PublicRoute>
    ),
  },
  {
    path: '/',
    element: (
      <ProtectedRoute>
        <MainLayout />
      </ProtectedRoute>
    ),
    children: [
      {
        index: true,
        element: <Navigate to="/dashboard" replace />,
      },
      {
        path: 'dashboard',
        element: <Dashboard />,
      },
      // 用户管理
      {
        path: 'users/list',
        element: <UserList />,
      },
      {
        path: 'users/roles',
        element: <RoleList />,
      },
      {
        path: 'users/permissions',
        element: <PermissionList />,
      },
      // 安全策略
      {
        path: 'security/dlp-rules',
        element: <DlpRuleList />,
      },
      {
        path: 'security/dictionaries',
        element: <DlpDictionaryList />,
      },
      {
        path: 'security/sensitive-ops',
        element: <SensitiveOpList />,
      },
      {
        path: 'security/policy-versions',
        element: <PolicyVersionList />,
      },
      // 审计日志
      {
        path: 'audit',
        element: <AuditLog />,
      },
      // 客户端管理
      {
        path: 'clients',
        element: <ClientList />,
      },
      // 扩展管理
      {
        path: 'extensions/skills',
        element: <SkillList />,
      },
      {
        path: 'extensions/plugins',
        element: <PluginList />,
      },
      // 系统配置
      {
        path: 'settings',
        element: <Settings />,
      },
      {
        path: 'client-config',
        element: <ClientConfig />,
      },
      {
        path: 'model-configs',
        element: <ModelConfigs />,
      },
      // 统计报表
      {
        path: 'reports',
        element: <Reports />,
      },
      // 部门管理
      {
        path: 'departments',
        element: <DepartmentList />,
      },
    ],
  },
  {
    path: '*',
    element: <Navigate to="/dashboard" replace />,
  },
]);
