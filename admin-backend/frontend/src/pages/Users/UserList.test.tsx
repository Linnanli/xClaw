import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { BrowserRouter } from 'react-router-dom';
import { UserList } from './UserList';
import { apiClient } from '../../api/client';

// Mock API client
vi.mock('../../api/client', () => ({
  apiClient: {
    get: vi.fn(),
    delete: vi.fn(),
  },
}));

// Mock Ant Design message
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

describe('UserList Component', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('Unit Tests - Component Rendering', () => {
    it('should render user list page', async () => {
      (apiClient.get as any).mockResolvedValue({ data: { users: [] } });

      render(
        <BrowserRouter>
          <UserList />
        </BrowserRouter>
      );

      await waitFor(() => {
        expect(screen.getByText('用户管理')).toBeInTheDocument();
      });
    });
  });

  describe('Integration Tests - Data Loading', () => {
    it('should load users from API', async () => {
      const mockUsers = [
        {
          id: '1',
          username: 'testuser',
          email: 'test@example.com',
          created_at: '2026-03-19T10:00:00Z',
          updated_at: '2026-03-19T10:00:00Z',
        },
      ];

      (apiClient.get as any).mockResolvedValue({ data: { users: mockUsers } });

      render(
        <BrowserRouter>
          <UserList />
        </BrowserRouter>
      );

      await waitFor(() => {
        expect(apiClient.get).toHaveBeenCalledWith('/users');
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
          <UserList />
        </BrowserRouter>
      );

      await waitFor(() => {
        expect(message.error).toHaveBeenCalled();
      });
    });
  });

  describe('Requirements Tests - User Management', () => {
    it('REQ-USER-001: should display user management page', async () => {
      (apiClient.get as any).mockResolvedValue({ data: { users: [] } });

      render(
        <BrowserRouter>
          <UserList />
        </BrowserRouter>
      );

      await waitFor(() => {
        expect(screen.getByText('用户管理')).toBeInTheDocument();
      });
    });
  });

  describe('Security Tests - Data Protection', () => {
    it('should not expose sensitive data', async () => {
      const mockUsers = [
        {
          id: '1',
          username: 'testuser',
          email: 'test@example.com',
          password_hash: 'secret',
          created_at: '2026-03-19T10:00:00Z',
          updated_at: '2026-03-19T10:00:00Z',
        },
      ];

      (apiClient.get as any).mockResolvedValue({ data: { users: mockUsers } });

      render(
        <BrowserRouter>
          <UserList />
        </BrowserRouter>
      );

      await waitFor(() => {
        expect(screen.queryByText('secret')).not.toBeInTheDocument();
      });
    });
  });

  describe('Code Coverage Tests - Edge Cases', () => {
    it('should handle empty user list', async () => {
      (apiClient.get as any).mockResolvedValue({ data: { users: [] } });

      render(
        <BrowserRouter>
          <UserList />
        </BrowserRouter>
      );

      await waitFor(() => {
        expect(apiClient.get).toHaveBeenCalled();
      });
    });
  });
});
