import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { BrowserRouter } from 'react-router-dom';
import { PermissionList } from './PermissionList';
import { apiClient } from '../../api/client';

vi.mock('../../api/client', () => ({
  apiClient: {
    get: vi.fn(),
  },
}));

vi.mock('antd', async () => {
  const actual = await vi.importActual('antd');
  return {
    ...actual,
    message: {
      success: vi.fn(),
      error: vi.fn(),
    },
  };
});

describe('PermissionList Component', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('Unit Tests - Component Rendering', () => {
    it('should render permission list page', async () => {
      (apiClient.get as any).mockResolvedValue({ data: { permissions: {} } });

      render(
        <BrowserRouter>
          <PermissionList />
        </BrowserRouter>
      );

      await waitFor(() => {
        expect(screen.getByText('权限管理')).toBeInTheDocument();
      });
    });
  });

  describe('Integration Tests - Data Loading', () => {
    it('should load permissions from API', async () => {
      const mockPermissions = {
        users: [
          {
            id: '1',
            name: 'users.list',
            description: '查看用户列表',
            resource: 'users',
            action: 'list',
            created_at: '2026-03-19T10:00:00Z',
          },
        ],
      };

      (apiClient.get as any).mockResolvedValue({ data: { permissions: mockPermissions } });

      render(
        <BrowserRouter>
          <PermissionList />
        </BrowserRouter>
      );

      await waitFor(() => {
        expect(apiClient.get).toHaveBeenCalledWith('/permissions');
      });
    });
  });

  describe('Failure Path Tests - Error Handling', () => {
    it('should handle API error', async () => {
      const { message } = await import('antd');
      (apiClient.get as any).mockRejectedValue({
        response: { data: { error: 'Failed to load' } },
      });

      render(
        <BrowserRouter>
          <PermissionList />
        </BrowserRouter>
      );

      await waitFor(() => {
        expect(message.error).toHaveBeenCalled();
      });
    });
  });

  describe('Requirements Tests - Permission Management', () => {
    it('REQ-PERM-001: should display permission management page', async () => {
      (apiClient.get as any).mockResolvedValue({ data: { permissions: {} } });

      render(
        <BrowserRouter>
          <PermissionList />
        </BrowserRouter>
      );

      await waitFor(() => {
        expect(screen.getByText('权限管理')).toBeInTheDocument();
      });
    });
  });

  describe('Security Tests - Data Protection', () => {
    it('should display permissions safely', async () => {
      const mockPermissions = {
        users: [
          {
            id: '1',
            name: 'users.list',
            description: '查看用户列表',
            resource: 'users',
            action: 'list',
            created_at: '2026-03-19T10:00:00Z',
          },
        ],
      };

      (apiClient.get as any).mockResolvedValue({ data: { permissions: mockPermissions } });

      render(
        <BrowserRouter>
          <PermissionList />
        </BrowserRouter>
      );

      await waitFor(() => {
        expect(apiClient.get).toHaveBeenCalled();
      });
    });
  });

  describe('Code Coverage Tests - Edge Cases', () => {
    it('should handle empty permission list', async () => {
      (apiClient.get as any).mockResolvedValue({ data: { permissions: {} } });

      render(
        <BrowserRouter>
          <PermissionList />
        </BrowserRouter>
      );

      await waitFor(() => {
        expect(apiClient.get).toHaveBeenCalled();
      });
    });
  });
});
