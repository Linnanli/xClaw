import React from 'react';
import { createBrowserRouter, Navigate } from 'react-router-dom';
import { Login } from '../pages/Login';
import { Dashboard } from '../pages/Dashboard';
import { ComingSoon } from '../pages/ComingSoon';
import { UserList } from '../pages/Users/UserList';
import { RoleList } from '../pages/Users/RoleList';
import { PermissionList } from '../pages/Users/PermissionList';
import { MainLayout } from '../components/Layout/MainLayout';
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
        element: <ComingSoon title="DLP 规则" />,
      },
      {
        path: 'security/sensitive-ops',
        element: <ComingSoon title="敏感操作" />,
      },
      {
        path: 'security/policy-versions',
        element: <ComingSoon title="策略版本" />,
      },
      // 审计日志
      {
        path: 'audit',
        element: <ComingSoon title="审计日志" />,
      },
      // 客户端管理
      {
        path: 'clients',
        element: <ComingSoon title="客户端管理" />,
      },
      // 扩展管理
      {
        path: 'extensions/skills',
        element: <ComingSoon title="技能管理" />,
      },
      {
        path: 'extensions/plugins',
        element: <ComingSoon title="插件管理" />,
      },
      // 系统配置
      {
        path: 'settings',
        element: <ComingSoon title="系统配置" />,
      },
      // 统计报表
      {
        path: 'reports',
        element: <ComingSoon title="统计报表" />,
      },
    ],
  },
  {
    path: '*',
    element: <Navigate to="/dashboard" replace />,
  },
]);
