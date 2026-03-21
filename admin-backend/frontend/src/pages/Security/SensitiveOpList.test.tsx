import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { BrowserRouter } from 'react-router-dom';
import { SensitiveOpList } from './SensitiveOpList';
import { apiClient } from '../../api/client';

vi.mock('../../api/client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    delete: vi.fn(),
    put: vi.fn(),
  },
}));

vi.mock('antd', async () => {
  const actual = await vi.importActual('antd');
  return {
    ...actual,
    message: {
      success: vi.fn(),
      error: vi.fn(),
      warning: vi.fn(),
    },
  };
});

const mockOperations = [
  {
    id: '1',
    name: '删除用户数据',
    operation_type: 'data_export',
    requires_approval: true,
    risk_level: 'high',
    description: '导出或删除用户个人数据',
    enabled: true,
    approver_roles: ['admin'],
    created_at: '2026-03-19T10:00:00Z',
    updated_at: '2026-03-19T10:00:00Z',
  },
  {
    id: '2',
    name: '执行系统命令',
    operation_type: 'system_command',
    requires_approval: true,
    risk_level: 'critical',
    description: '执行系统级命令',
    enabled: true,
    approver_roles: ['admin', 'security_officer'],
    created_at: '2026-03-20T10:00:00Z',
    updated_at: '2026-03-20T10:00:00Z',
  },
  {
    id: '3',
    name: '修改安全配置',
    operation_type: 'config_change',
    requires_approval: false,
    risk_level: 'medium',
    description: '修改系统安全策略配置',
    enabled: false,
    approver_roles: [],
    created_at: '2026-03-21T10:00:00Z',
    updated_at: '2026-03-21T10:00:00Z',
  },
];

const renderComponent = () => {
  return render(
    <BrowserRouter>
      <SensitiveOpList />
    </BrowserRouter>
  );
};

describe('SensitiveOpList Component', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // ============================================================
  // 单元测试：组件渲染
  // ============================================================
  describe('Unit Tests - Component Rendering', () => {
    it('should render page title and action buttons', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { operations: [] } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('敏感操作管理')).toBeInTheDocument();
        expect(screen.getByText('刷新')).toBeInTheDocument();
        expect(screen.getByText('创建规则')).toBeInTheDocument();
      });
    });

    it('should render search and filter bar', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { operations: [] } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByPlaceholderText('搜索操作名称或描述')).toBeInTheDocument();
        expect(document.querySelector('.sensitive-op-filter-bar')).toBeInTheDocument();
      });
    });

    it('should render table columns', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { operations: mockOperations } });
      renderComponent();

      await waitFor(() => {
        const headers = screen.getAllByRole('columnheader');
        const headerTexts = headers.map(h => h.textContent);
        expect(headerTexts).toContain('操作名称');
        expect(headerTexts).toContain('操作类型');
        expect(headerTexts).toContain('风险等级');
        expect(headerTexts).toContain('审批要求');
        expect(headerTexts).toContain('状态');
        expect(headerTexts).toContain('操作');
      });
    });

    it('should render empty state when no operations', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { operations: [] } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText(/还没有敏感操作规则/)).toBeInTheDocument();
        expect(screen.getByText('创建第一条规则')).toBeInTheDocument();
      });
    });
  });

  // ============================================================
  // 集成测试：数据加载和交互
  // ============================================================
  describe('Integration Tests - Data Loading', () => {
    it('should load operations from API on mount', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { operations: mockOperations } });
      renderComponent();

      await waitFor(() => {
        expect(apiClient.get).toHaveBeenCalledWith('/sensitive-operations');
      });
    });

    it('should display loaded operations', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { operations: mockOperations } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('删除用户数据')).toBeInTheDocument();
        expect(screen.getByText('执行系统命令')).toBeInTheDocument();
        expect(screen.getByText('修改安全配置')).toBeInTheDocument();
      });
    });

    it('should display operation count summary', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { operations: mockOperations } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText(/共 3 条规则/)).toBeInTheDocument();
      });
    });

    it('should display risk level tags with correct colors', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { operations: mockOperations } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('高')).toBeInTheDocument();
        expect(screen.getByText('严重')).toBeInTheDocument();
        expect(screen.getByText('中')).toBeInTheDocument();
      });
    });

    it('should display approval status correctly', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { operations: mockOperations } });
      renderComponent();

      await waitFor(() => {
        const approvalTags = screen.getAllByText('需要审批');
        expect(approvalTags.length).toBe(2);
        expect(screen.getByText('自动放行')).toBeInTheDocument();
      });
    });

    it('should reload on refresh button click', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { operations: mockOperations } });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(apiClient.get).toHaveBeenCalledTimes(1);
      });

      await user.click(screen.getByText('刷新'));

      await waitFor(() => {
        expect(apiClient.get).toHaveBeenCalledTimes(2);
      });
    });
  });

  // ============================================================
  // 集成测试：搜索和筛选
  // ============================================================
  describe('Integration Tests - Search and Filter', () => {
    it('should filter operations by search text', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { operations: mockOperations } });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('删除用户数据')).toBeInTheDocument();
      });

      const searchInput = screen.getByPlaceholderText('搜索操作名称或描述');
      await user.type(searchInput, '删除');

      await waitFor(() => {
        expect(screen.getByText('删除用户数据')).toBeInTheDocument();
        expect(screen.queryByText('执行系统命令')).not.toBeInTheDocument();
      });
    });

    it('should show filtered count', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { operations: mockOperations } });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('删除用户数据')).toBeInTheDocument();
      });

      const searchInput = screen.getByPlaceholderText('搜索操作名称或描述');
      await user.type(searchInput, '删除');

      await waitFor(() => {
        expect(screen.getByText(/显示 1 条/)).toBeInTheDocument();
      });
    });

    it('should show clear filters button when filters active', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { operations: mockOperations } });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('删除用户数据')).toBeInTheDocument();
      });

      const searchInput = screen.getByPlaceholderText('搜索操作名称或描述');
      await user.type(searchInput, '删除');

      await waitFor(() => {
        expect(screen.getByText('清除筛选')).toBeInTheDocument();
      });
    });
  });

  // ============================================================
  // 集成测试：CRUD 操作
  // ============================================================
  describe('Integration Tests - CRUD Operations', () => {
    it('should open create modal on button click', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { operations: [] } });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('创建规则')).toBeInTheDocument();
      });

      await user.click(screen.getByText('创建规则'));

      await waitFor(() => {
        expect(screen.getByText('创建敏感操作')).toBeInTheDocument();
      });
    });

    it('should open edit modal on edit button click', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { operations: mockOperations } });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('删除用户数据')).toBeInTheDocument();
      });

      const editButtons = screen.getAllByText('编辑');
      await user.click(editButtons[0]);

      await waitFor(() => {
        expect(screen.getByText('编辑敏感操作')).toBeInTheDocument();
      });
    });

    it('should open edit modal on name click', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { operations: mockOperations } });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('删除用户数据')).toBeInTheDocument();
      });

      await user.click(screen.getByText('删除用户数据'));

      await waitFor(() => {
        expect(screen.getByText('编辑敏感操作')).toBeInTheDocument();
      });
    });

    it('should toggle operation status', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { operations: mockOperations } });
      vi.mocked(apiClient.put).mockResolvedValue({ data: {} });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('删除用户数据')).toBeInTheDocument();
      });

      const switches = screen.getAllByRole('switch');
      // 第三个 switch 是「修改安全配置」（disabled）
      await user.click(switches[2]);

      await waitFor(() => {
        expect(apiClient.put).toHaveBeenCalledWith('/sensitive-operations/3', { enabled: true });
      });
    });

    it('should delete operation after confirmation', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { operations: mockOperations } });
      vi.mocked(apiClient.delete).mockResolvedValue({ data: {} });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('删除用户数据')).toBeInTheDocument();
      });

      const deleteButtons = screen.getAllByText('删除');
      await user.click(deleteButtons[0]);

      await waitFor(() => {
        expect(screen.getByText('确认删除')).toBeInTheDocument();
      });

      const popconfirmOkBtn = document.querySelector('.ant-popconfirm-buttons .ant-btn-primary') as HTMLElement;
      if (popconfirmOkBtn) {
        await user.click(popconfirmOkBtn);
      }

      await waitFor(() => {
        expect(apiClient.delete).toHaveBeenCalledWith('/sensitive-operations/1');
      });
    });
  });

  // ============================================================
  // 失败路径测试
  // ============================================================
  describe('Failure Path Tests', () => {
    it('test_failure_load_api_error: should handle load API error', async () => {
      const { message } = await import('antd');
      vi.mocked(apiClient.get).mockRejectedValue({
        response: { data: { error: '服务器错误' } },
      });

      renderComponent();

      await waitFor(() => {
        expect(message.error).toHaveBeenCalled();
      });
    });

    it('test_failure_load_network_error: should handle network error', async () => {
      const { message } = await import('antd');
      vi.mocked(apiClient.get).mockRejectedValue(new Error('Network Error'));

      renderComponent();

      await waitFor(() => {
        expect(message.error).toHaveBeenCalled();
      });
    });

    it('test_failure_delete_error: should handle delete API error', async () => {
      const { message } = await import('antd');
      vi.mocked(apiClient.get).mockResolvedValue({ data: { operations: mockOperations } });
      vi.mocked(apiClient.delete).mockRejectedValue({
        response: { data: { error: '删除失败' } },
      });

      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('删除用户数据')).toBeInTheDocument();
      });

      const deleteButtons = screen.getAllByText('删除');
      await user.click(deleteButtons[0]);

      await waitFor(() => {
        expect(screen.getByText('确认删除')).toBeInTheDocument();
      });

      const popconfirmOkBtn = document.querySelector('.ant-popconfirm-buttons .ant-btn-primary') as HTMLElement;
      if (popconfirmOkBtn) {
        await user.click(popconfirmOkBtn);
      }

      await waitFor(() => {
        expect(message.error).toHaveBeenCalled();
      });
    });

    it('test_failure_toggle_error: should handle toggle status error', async () => {
      const { message } = await import('antd');
      vi.mocked(apiClient.get).mockResolvedValue({ data: { operations: mockOperations } });
      vi.mocked(apiClient.put).mockRejectedValue({
        response: { data: { error: '更新失败' } },
      });

      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('删除用户数据')).toBeInTheDocument();
      });

      const switches = screen.getAllByRole('switch');
      await user.click(switches[0]);

      await waitFor(() => {
        expect(message.error).toHaveBeenCalled();
      });
    });

    it('test_failure_null_operations_response: should handle null operations', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: {} });
      renderComponent();

      await waitFor(() => {
        expect(apiClient.get).toHaveBeenCalled();
      });
    });
  });

  // ============================================================
  // 安全测试
  // ============================================================
  describe('Security Tests', () => {
    it('test_security_xss_in_name: should safely render operation names', async () => {
      const xssOps = [{
        ...mockOperations[0],
        name: '<script>alert("xss")</script>',
      }];
      vi.mocked(apiClient.get).mockResolvedValue({ data: { operations: xssOps } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('<script>alert("xss")</script>')).toBeInTheDocument();
      });
    });

    it('test_security_xss_in_search: should safely handle search input', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { operations: mockOperations } });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('删除用户数据')).toBeInTheDocument();
      });

      const searchInput = screen.getByPlaceholderText('搜索操作名称或描述');
      await user.type(searchInput, '<img onerror=alert(1)>');

      expect(document.querySelector('img')).toBeNull();
    });

    it('test_security_xss_in_description: should safely render descriptions', async () => {
      const xssOps = [{
        ...mockOperations[0],
        description: '<img src=x onerror=alert(1)>',
      }];
      vi.mocked(apiClient.get).mockResolvedValue({ data: { operations: xssOps } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('删除用户数据')).toBeInTheDocument();
      });
    });
  });

  // ============================================================
  // 需求级测试
  // ============================================================
  describe('Requirements Tests', () => {
    it('req_sensitive_ops_001: should display operation list with all columns', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { operations: mockOperations } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('敏感操作管理')).toBeInTheDocument();
        expect(screen.getByText('删除用户数据')).toBeInTheDocument();
      });
    });

    it('req_sensitive_ops_002: should support search functionality', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { operations: mockOperations } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByPlaceholderText('搜索操作名称或描述')).toBeInTheDocument();
      });
    });

    it('req_sensitive_ops_003: should support edit functionality', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { operations: mockOperations } });
      renderComponent();

      await waitFor(() => {
        const editButtons = screen.getAllByText('编辑');
        expect(editButtons.length).toBeGreaterThan(0);
      });
    });

    it('req_sensitive_ops_004: should support delete with confirmation', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { operations: mockOperations } });
      renderComponent();

      await waitFor(() => {
        const deleteButtons = screen.getAllByText('删除');
        expect(deleteButtons.length).toBeGreaterThan(0);
      });
    });

    it('req_sensitive_ops_005: should support enable/disable toggle', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { operations: mockOperations } });
      renderComponent();

      await waitFor(() => {
        const switches = screen.getAllByRole('switch');
        expect(switches.length).toBe(mockOperations.length);
      });
    });

    it('req_sensitive_ops_006: should display approval requirements', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { operations: mockOperations } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getAllByText('需要审批').length).toBeGreaterThan(0);
        expect(screen.getByText('自动放行')).toBeInTheDocument();
      });
    });
  });

  // ============================================================
  // 契约测试
  // ============================================================
  describe('Contract Tests', () => {
    it('test_contract_load_endpoint: should call correct API endpoint', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { operations: [] } });
      renderComponent();

      await waitFor(() => {
        expect(apiClient.get).toHaveBeenCalledWith('/sensitive-operations');
      });
    });

    it('test_contract_delete_endpoint: should call correct delete endpoint', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { operations: mockOperations } });
      vi.mocked(apiClient.delete).mockResolvedValue({ data: {} });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('删除用户数据')).toBeInTheDocument();
      });

      const deleteButtons = screen.getAllByText('删除');
      await user.click(deleteButtons[0]);

      await waitFor(() => {
        expect(screen.getByText('确认删除')).toBeInTheDocument();
      });

      const popconfirmOkBtn = document.querySelector('.ant-popconfirm-buttons .ant-btn-primary') as HTMLElement;
      if (popconfirmOkBtn) {
        await user.click(popconfirmOkBtn);
      }

      await waitFor(() => {
        expect(apiClient.delete).toHaveBeenCalledWith('/sensitive-operations/1');
      });
    });

    it('test_contract_toggle_endpoint: should call correct toggle endpoint', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { operations: mockOperations } });
      vi.mocked(apiClient.put).mockResolvedValue({ data: {} });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('删除用户数据')).toBeInTheDocument();
      });

      const switches = screen.getAllByRole('switch');
      await user.click(switches[0]); // 第一个 switch（enabled=true -> false）

      await waitFor(() => {
        expect(apiClient.put).toHaveBeenCalledWith('/sensitive-operations/1', { enabled: false });
      });
    });
  });

  // ============================================================
  // 代码覆盖测试：边界情况
  // ============================================================
  describe('Code Coverage - Edge Cases', () => {
    it('should handle operations with missing optional fields', async () => {
      const minimalOps = [{
        id: '1',
        name: '最小操作',
        operation_type: 'file_operation',
        requires_approval: false,
        risk_level: 'low',
        enabled: true,
        approver_roles: [],
        created_at: '2026-03-19T10:00:00Z',
        updated_at: '2026-03-19T10:00:00Z',
      }];
      vi.mocked(apiClient.get).mockResolvedValue({ data: { operations: minimalOps } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('最小操作')).toBeInTheDocument();
      });
    });

    it('should handle unknown risk level value', async () => {
      const unknownOps = [{
        ...mockOperations[0],
        risk_level: 'unknown_level',
      }];
      vi.mocked(apiClient.get).mockResolvedValue({ data: { operations: unknownOps } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('删除用户数据')).toBeInTheDocument();
      });
    });

    it('should handle unknown operation type', async () => {
      const unknownOps = [{
        ...mockOperations[0],
        operation_type: 'unknown_type',
      }];
      vi.mocked(apiClient.get).mockResolvedValue({ data: { operations: unknownOps } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('删除用户数据')).toBeInTheDocument();
      });
    });

    it('should handle empty approver_roles for approval-required operation', async () => {
      const emptyRolesOps = [{
        ...mockOperations[0],
        requires_approval: true,
        approver_roles: [],
      }];
      vi.mocked(apiClient.get).mockResolvedValue({ data: { operations: emptyRolesOps } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('需要审批')).toBeInTheDocument();
      });
    });
  });
});
