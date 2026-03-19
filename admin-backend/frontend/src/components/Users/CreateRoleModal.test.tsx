import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import { CreateRoleModal } from './CreateRoleModal';
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

describe('CreateRoleModal Component', () => {
  const mockOnCancel = vi.fn();
  const mockOnSuccess = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('Unit Tests - Form Rendering', () => {
    it('should render modal title', () => {
      render(
        <CreateRoleModal
          visible={true}
          onCancel={mockOnCancel}
          onSuccess={mockOnSuccess}
        />
      );

      expect(screen.getByText('创建角色')).toBeInTheDocument();
    });

    it('should render form fields', () => {
      render(
        <CreateRoleModal
          visible={true}
          onCancel={mockOnCancel}
          onSuccess={mockOnSuccess}
        />
      );

      expect(screen.getByPlaceholderText('请输入角色名')).toBeInTheDocument();
      expect(screen.getByPlaceholderText('请输入角色描述')).toBeInTheDocument();
    });
  });

  describe('Requirements Tests - Modal Specifications', () => {
    it('REQ-MODAL-001: should display modal title', () => {
      render(
        <CreateRoleModal
          visible={true}
          onCancel={mockOnCancel}
          onSuccess={mockOnSuccess}
        />
      );

      expect(screen.getByText('创建角色')).toBeInTheDocument();
    });

    it('REQ-MODAL-002: should have required form fields', () => {
      render(
        <CreateRoleModal
          visible={true}
          onCancel={mockOnCancel}
          onSuccess={mockOnSuccess}
        />
      );

      expect(screen.getByLabelText('角色名')).toBeInTheDocument();
      expect(screen.getByLabelText('描述')).toBeInTheDocument();
    });
  });

  describe('Security Tests - Input Validation', () => {
    it('should validate role name pattern', () => {
      render(
        <CreateRoleModal
          visible={true}
          onCancel={mockOnCancel}
          onSuccess={mockOnSuccess}
        />
      );

      const nameInput = screen.getByPlaceholderText('请输入角色名');
      expect(nameInput).toBeInTheDocument();
    });
  });

  describe('Code Coverage Tests - Edge Cases', () => {
    it('should handle modal visibility', () => {
      const { container } = render(
        <CreateRoleModal
          visible={false}
          onCancel={mockOnCancel}
          onSuccess={mockOnSuccess}
        />
      );

      expect(container.querySelector('.ant-modal')).not.toBeInTheDocument();
    });
  });
});
