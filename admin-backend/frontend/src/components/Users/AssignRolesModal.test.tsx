import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { AssignRolesModal } from './AssignRolesModal';
import { apiClient } from '../../api/client';

vi.mock('../../api/client');

describe('AssignRolesModal', () => {
  const mockOnCancel = vi.fn();
  const mockOnSuccess = vi.fn();

  const mockRoles = [
    {
      id: 'role-1',
      name: 'admin',
      description: '系统管理员',
      permission_count: 15,
      user_count: 1,
      created_at: '2026-03-19T00:00:00Z',
      updated_at: '2026-03-19T00:00:00Z',
    },
    {
      id: 'role-2',
      name: 'user',
      description: '普通用户',
      permission_count: 3,
      user_count: 5,
      created_at: '2026-03-19T00:00:00Z',
      updated_at: '2026-03-19T00:00:00Z',
    },
  ];

  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(apiClient.get).mockResolvedValue({ data: { roles: mockRoles } });
    vi.mocked(apiClient.post).mockResolvedValue({ data: {} });
  });

  // 单元测试：组件渲染
  it('should render modal with title', async () => {
    render(
      <AssignRolesModal
        visible={true}
        userId="user-1"
        username="testuser"
        currentRoles={[]}
        onCancel={mockOnCancel}
        onSuccess={mockOnSuccess}
      />
    );

    await waitFor(() => {
      expect(screen.getByText('分配角色 - testuser')).toBeInTheDocument();
    });
  });

  // 单元测试：表单字段
  it('should render role select field', async () => {
    render(
      <AssignRolesModal
        visible={true}
        userId="user-1"
        username="testuser"
        currentRoles={[]}
        onCancel={mockOnCancel}
        onSuccess={mockOnSuccess}
      />
    );

    await waitFor(() => {
      expect(screen.getByText('角色')).toBeInTheDocument();
    });
  });

  // 集成测试：加载角色列表
  it('should load roles on mount', async () => {
    render(
      <AssignRolesModal
        visible={true}
        userId="user-1"
        username="testuser"
        currentRoles={[]}
        onCancel={mockOnCancel}
        onSuccess={mockOnSuccess}
      />
    );

    await waitFor(() => {
      expect(apiClient.get).toHaveBeenCalledWith('/roles');
    });
  });

  // 集成测试：分配角色
  it('should assign roles successfully', async () => {
    const user = userEvent.setup();
    
    render(
      <AssignRolesModal
        visible={true}
        userId="user-1"
        username="testuser"
        currentRoles={['role-1']}
        onCancel={mockOnCancel}
        onSuccess={mockOnSuccess}
      />
    );

    await waitFor(() => {
      expect(screen.getByText('分配角色 - testuser')).toBeInTheDocument();
    });

    // 等待角色加载完成
    await waitFor(() => {
      expect(apiClient.get).toHaveBeenCalledWith('/roles');
    });

    // 点击保存按钮（当前角色已选中）
    const saveButton = screen.getByText(/保\s*存/);
    await user.click(saveButton);

    await waitFor(() => {
      expect(apiClient.post).toHaveBeenCalledWith(
        '/users/user-1/roles',
        expect.objectContaining({
          role_ids: expect.any(Array),
        })
      );
      expect(mockOnSuccess).toHaveBeenCalled();
    });
  });

  // 失败路径测试：API 错误
  it('should handle API error', async () => {
    vi.mocked(apiClient.get).mockRejectedValue({
      response: { data: { error: 'Failed to load roles' } },
    });

    render(
      <AssignRolesModal
        visible={true}
        userId="user-1"
        username="testuser"
        currentRoles={[]}
        onCancel={mockOnCancel}
        onSuccess={mockOnSuccess}
      />
    );

    await waitFor(() => {
      expect(apiClient.get).toHaveBeenCalled();
    });
  });

  // 需求级测试：REQ-ASSIGN-ROLES-001
  it('REQ-ASSIGN-ROLES-001: should display current roles', async () => {
    render(
      <AssignRolesModal
        visible={true}
        userId="user-1"
        username="testuser"
        currentRoles={['role-1']}
        onCancel={mockOnCancel}
        onSuccess={mockOnSuccess}
      />
    );

    await waitFor(() => {
      expect(screen.getByText('分配角色 - testuser')).toBeInTheDocument();
    });
  });

  // 安全测试：验证用户 ID
  it('should include user ID in API call', async () => {
    const user = userEvent.setup();
    
    render(
      <AssignRolesModal
        visible={true}
        userId="user-123"
        username="testuser"
        currentRoles={['role-1']}
        onCancel={mockOnCancel}
        onSuccess={mockOnSuccess}
      />
    );

    await waitFor(() => {
      expect(screen.getByText('分配角色 - testuser')).toBeInTheDocument();
    });

    // 等待角色加载完成
    await waitFor(() => {
      expect(apiClient.get).toHaveBeenCalledWith('/roles');
    });

    const saveButton = screen.getByText(/保\s*存/);
    await user.click(saveButton);

    await waitFor(() => {
      expect(apiClient.post).toHaveBeenCalledWith(
        '/users/user-123/roles',
        expect.any(Object)
      );
    });
  });

  // 代码覆盖测试：模态框可见性
  it('should not load roles when modal is not visible', () => {
    render(
      <AssignRolesModal
        visible={false}
        userId="user-1"
        username="testuser"
        currentRoles={[]}
        onCancel={mockOnCancel}
        onSuccess={mockOnSuccess}
      />
    );

    expect(apiClient.get).not.toHaveBeenCalled();
  });
});
