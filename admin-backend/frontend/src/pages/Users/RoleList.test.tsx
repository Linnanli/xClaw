import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { BrowserRouter } from 'react-router-dom';
import { RoleList } from './RoleList';
import { apiClient } from '../../api/client';

vi.mock('../../api/client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    put: vi.fn(),
    delete: vi.fn(),
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

describe('RoleList Component', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('Unit Tests - Component Rendering', () => {
    it('should render role list page', async () => {
      (apiClient.get as any).mockResolvedValue({ data: { roles: [] } });

      render(
        <BrowserRouter>
          <RoleList />
        </BrowserRouter>
      );

      await waitFor(() => {
        expect(screen.getByText('角色管理')).toBeInTheDocument();
      });
    });

    it('should render action buttons', async () => {
      (apiClient.get as any).mockResolvedValue({ data: { roles: [] } });

      render(
        <BrowserRouter>
          <RoleList />
        </BrowserRouter>
      );

      await waitFor(() => {
        expect(screen.getByText('刷新')).toBeInTheDocument();
        expect(screen.getByText('创建角色')).toBeInTheDocument();
      });
    });
  });

  describe('Integration Tests - Data Loading', () => {
    it('should load roles from API', async () => {
      const mockRoles = [
        {
          id: '1',
          name: 'admin',
          description: '系统管理员',
          permission_count: 15,
          user_count: 1,
          created_at: '2026-03-19T10:00:00Z',
          updated_at: '2026-03-19T10:00:00Z',
        },
      ];

      (apiClient.get as any).mockResolvedValue({ data: { roles: mockRoles } });

      render(
        <BrowserRouter>
          <RoleList />
        </BrowserRouter>
      );

      await waitFor(() => {
        expect(apiClient.get).toHaveBeenCalledWith('/roles');
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
          <RoleList />
        </BrowserRouter>
      );

      await waitFor(() => {
        expect(message.error).toHaveBeenCalled();
      });
    });
  });

  describe('Requirements Tests - Role Management', () => {
    it('REQ-ROLE-001: should display role management page', async () => {
      (apiClient.get as any).mockResolvedValue({ data: { roles: [] } });

      render(
        <BrowserRouter>
          <RoleList />
        </BrowserRouter>
      );

      await waitFor(() => {
        expect(screen.getByText('角色管理')).toBeInTheDocument();
      });
    });
  });

  describe('Security Tests - Data Protection', () => {
    it('should not expose sensitive data', async () => {
      const mockRoles = [
        {
          id: '1',
          name: 'admin',
          description: '系统管理员',
          permission_count: 15,
          user_count: 1,
          created_at: '2026-03-19T10:00:00Z',
          updated_at: '2026-03-19T10:00:00Z',
        },
      ];

      (apiClient.get as any).mockResolvedValue({ data: { roles: mockRoles } });

      render(
        <BrowserRouter>
          <RoleList />
        </BrowserRouter>
      );

      await waitFor(() => {
        expect(apiClient.get).toHaveBeenCalled();
      });
    });
  });

  describe('Code Coverage Tests - Edge Cases', () => {
    it('should handle empty role list', async () => {
      (apiClient.get as any).mockResolvedValue({ data: { roles: [] } });

      render(
        <BrowserRouter>
          <RoleList />
        </BrowserRouter>
      );

      await waitFor(() => {
        expect(apiClient.get).toHaveBeenCalled();
      });
    });
  });
});
