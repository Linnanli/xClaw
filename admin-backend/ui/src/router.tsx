import { createBrowserRouter, Navigate } from 'react-router-dom'
import { AppLayout } from '@/components/layout/app-layout'
import LoginPage from '@/pages/login'
import DashboardPage from '@/pages/dashboard'
import UsersPage from '@/pages/users'
import ClientsPage from '@/pages/clients'
import SecurityPage from '@/pages/security'
import AlertsPage from '@/pages/alerts'
import ConversationsPage from '@/pages/conversations'
import KnowledgeBasesPage from '@/pages/knowledge-bases'
import QuotaPage from '@/pages/quota'
import ApprovalsPage from '@/pages/approvals'
import CompliancePage from '@/pages/compliance'
import WatermarkPage from '@/pages/watermark'
import SettingsPage from '@/pages/settings'
import DepartmentsPage from '@/pages/departments'
import ReportsPage from '@/pages/reports'
import ModelConfigsPage from '@/pages/model-configs'
import AuditLogsPage from '@/pages/audit-logs'
import ExtensionsPage from '@/pages/extensions'
import { useAuthStore } from '@/stores/auth'

function ProtectedRoute({ children }: { children: React.ReactNode }) {
  const isAuthenticated = useAuthStore((s) => s.isAuthenticated)
  if (!isAuthenticated) {
    return <Navigate to="/login" replace />
  }
  return <>{children}</>
}

function GuestRoute({ children }: { children: React.ReactNode }) {
  const isAuthenticated = useAuthStore((s) => s.isAuthenticated)
  if (isAuthenticated) {
    return <Navigate to="/" replace />
  }
  return <>{children}</>
}

export const router = createBrowserRouter([
  {
    path: '/login',
    element: <GuestRoute><LoginPage /></GuestRoute>,
  },
  {
    path: '/',
    element: (
      <ProtectedRoute>
        <AppLayout />
      </ProtectedRoute>
    ),
    children: [
      { index: true, element: <DashboardPage /> },
      { path: 'users', element: <UsersPage /> },
      { path: 'clients', element: <ClientsPage /> },
      { path: 'security', element: <SecurityPage /> },
      { path: 'alerts', element: <AlertsPage /> },
      { path: 'conversations', element: <ConversationsPage /> },
      { path: 'knowledge-bases', element: <KnowledgeBasesPage /> },
      { path: 'quota', element: <QuotaPage /> },
      { path: 'approvals', element: <ApprovalsPage /> },
      { path: 'compliance', element: <CompliancePage /> },
      { path: 'watermark', element: <WatermarkPage /> },
      { path: 'settings', element: <SettingsPage /> },
      { path: 'departments', element: <DepartmentsPage /> },
      { path: 'reports', element: <ReportsPage /> },
      { path: 'model-configs', element: <ModelConfigsPage /> },
      { path: 'audit-logs', element: <AuditLogsPage /> },
      { path: 'extensions', element: <ExtensionsPage /> },
    ],
  },
])
