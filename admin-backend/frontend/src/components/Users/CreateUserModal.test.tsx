import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import { CreateUserModal } from './CreateUserModal';
import { apiClient } from '../../api/client';

vi.mock('../../api/client', () => ({
  apiClient: {
    post: vi.fn(),
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

describe('CreateUserModal Component', () => {
  const mockOnCancel = vi.fn();
  const mockOnSuccess = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('Unit Tests', () => {
    it('should render modal title', () => {
      render(
        <CreateUserModal
          visible={true}
          onCancel={mockOnCancel}
          onSuccess={mockOnSuccess}
        />
      );

      expect(screen.getByText('创建用户')).toBeInTheDocument();
    });

    it('should render form fields', () => {
      render(
        <CreateUserModal
          visible={true}
          onCancel={mockOnCancel}
          onSuccess={mockOnSuccess}
        />
      );

      expect(screen.getByPlaceholderText('请输入用户名')).toBeInTheDocument();
      expect(screen.getByPlaceholderText('请输入邮箱')).toBeInTheDocument();
    });
  });

  describe('Security Tests', () => {
    it('should use password input type', () => {
      render(
        <CreateUserModal
          visible={true}
          onCancel={mockOnCancel}
          onSuccess={mockOnSuccess}
        />
      );

      const passwordInput = screen.getByPlaceholderText('请输入密码');
      expect(passwordInput).toHaveAttribute('type', 'password');
    });
  });
});
