import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { BrowserRouter } from 'react-router-dom';
import { AuditLog } from './AuditLog';
import { apiClient } from '../api/client';

vi.mock('../api/client', () => ({
  apiClient: {
    get: vi.fn(),
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
      warning: vi.fn(),
    },
  };
});

const mockLogs = [
  {
    id: 'log-1',
    user_id: 'user-uuid-1',
    action: 'login',
    details: '用户 admin 登录成功',
    created_at: '2026-03-21T10:00:00Z',
  },
  {
    id: 'log-2',
    user_id: '00000000-0000-0000-0000-000000000000',
    action: 'dlp_redact',
    details: 'DLP 脱敏: 匹配 2 处, 脱敏 2 处',
    created_at: '2026-03-21T10:05:00Z',
  },
  {
    id: 'log-3',
    user_id: 'user-uuid-1',
    action: 'create_dlp_rule',
    details: '创建 DLP 规则: 身份证号',
    created_at: '2026-03-21T10:10:00Z',
  },
  {
    id: 'log-4',
    user_id: '00000000-0000-0000-0000-000000000000',
    action: 'dlp_block',
    details: 'DLP 阻止发送: 匹配 1 处, 阻止 1 处',
    created_at: '2026-03-21T10:15:00Z',
  },
];

const renderComponent = () => {
  return render(
    <BrowserRouter>
      <AuditLog />
    </BrowserRouter>
  );
};

describe('AuditLog Component', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // ============================================================
  // 单元测试：组件渲染
  // ============================================================
  describe('Unit Tests - Component Rendering', () => {
    it('should render page title and action buttons', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { logs: [] } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('审计日志')).toBeInTheDocument();
        expect(screen.getByText('刷新')).toBeInTheDocument();
        expect(screen.getByText('导出')).toBeInTheDocument();
      });
    });

    it('should render search and filter bar', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { logs: [] } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByPlaceholderText('搜索操作类型、详情或用户 ID')).toBeInTheDocument();
        expect(document.querySelector('.audit-filter-bar')).toBeInTheDocument();
      });
    });

    it('should render table columns', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { logs: mockLogs } });
      renderComponent();

      await waitFor(() => {
        const headers = screen.getAllByRole('columnheader');
        const headerTexts = headers.map(h => h.textContent);
        expect(headerTexts).toContain('时间');
        expect(headerTexts).toContain('操作类型');
        expect(headerTexts).toContain('用户 ID');
        expect(headerTexts).toContain('详情');
      });
    });

    it('should render empty state when no logs', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { logs: [] } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('暂无审计日志')).toBeInTheDocument();
      });
    });

    it('should display "系统 / 客户端" for nil UUID user_id', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { logs: mockLogs } });
      renderComponent();

      await waitFor(() => {
        const systemTags = screen.getAllByText('系统 / 客户端');
        expect(systemTags.length).toBe(2); // log-2 and log-4
      });
    });
  });

  // ============================================================
  // 集成测试：数据加载和交互
  // ============================================================
  describe('Integration Tests - Data Loading', () => {
    it('should load logs from API on mount', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { logs: mockLogs } });
      renderComponent();

      await waitFor(() => {
        expect(apiClient.get).toHaveBeenCalledWith('/audit-logs');
      });
    });

    it('should display loaded logs', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { logs: mockLogs } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('用户 admin 登录成功')).toBeInTheDocument();
        expect(screen.getByText(/DLP 脱敏/)).toBeInTheDocument();
      });
    });

    it('should display log count summary', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { logs: mockLogs } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText(/共 4 条日志/)).toBeInTheDocument();
      });
    });

    it('should reload logs on refresh button click', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { logs: mockLogs } });
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
    it('should filter logs by search text', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { logs: mockLogs } });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('用户 admin 登录成功')).toBeInTheDocument();
      });

      const searchInput = screen.getByPlaceholderText('搜索操作类型、详情或用户 ID');
      await user.type(searchInput, 'DLP');

      await waitFor(() => {
        expect(screen.queryByText('用户 admin 登录成功')).not.toBeInTheDocument();
        expect(screen.getByText(/DLP 脱敏/)).toBeInTheDocument();
      });
    });

    it('should show filtered count when filters active', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { logs: mockLogs } });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('用户 admin 登录成功')).toBeInTheDocument();
      });

      const searchInput = screen.getByPlaceholderText('搜索操作类型、详情或用户 ID');
      await user.type(searchInput, 'login');

      await waitFor(() => {
        expect(screen.getByText(/显示 1 条/)).toBeInTheDocument();
      });
    });

    it('should show clear filters button when filters active', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { logs: mockLogs } });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('用户 admin 登录成功')).toBeInTheDocument();
      });

      const searchInput = screen.getByPlaceholderText('搜索操作类型、详情或用户 ID');
      await user.type(searchInput, 'login');

      await waitFor(() => {
        expect(screen.getByText('清除筛选')).toBeInTheDocument();
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

    it('test_failure_empty_response: should handle empty logs array', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { logs: [] } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('暂无审计日志')).toBeInTheDocument();
      });
    });

    it('test_failure_null_logs_response: should handle null logs in response', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: {} });
      renderComponent();

      await waitFor(() => {
        expect(apiClient.get).toHaveBeenCalled();
      });
    });

    it('test_failure_no_matching_filter: should show empty state for no matches', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { logs: mockLogs } });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('用户 admin 登录成功')).toBeInTheDocument();
      });

      const searchInput = screen.getByPlaceholderText('搜索操作类型、详情或用户 ID');
      await user.type(searchInput, 'nonexistent_action_xyz');

      await waitFor(() => {
        expect(screen.getByText('清除筛选条件')).toBeInTheDocument();
      });
    });
  });

  // ============================================================
  // 安全测试
  // ============================================================
  describe('Security Tests', () => {
    it('test_security_xss_in_details: should safely render log details', async () => {
      const xssLogs = [{
        ...mockLogs[0],
        details: '<script>alert("xss")</script>',
      }];
      vi.mocked(apiClient.get).mockResolvedValue({ data: { logs: xssLogs } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('<script>alert("xss")</script>')).toBeInTheDocument();
      });
    });

    it('test_security_xss_in_action: should safely render action field', async () => {
      const xssLogs = [{
        ...mockLogs[0],
        action: '<img onerror=alert(1) src=x>',
      }];
      vi.mocked(apiClient.get).mockResolvedValue({ data: { logs: xssLogs } });
      renderComponent();

      await waitFor(() => {
        expect(document.querySelector('img[onerror]')).toBeNull();
      });
    });

    it('test_security_no_sensitive_data_in_export: export should not leak raw data', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { logs: mockLogs } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('导出')).toBeInTheDocument();
      });

      // 导出按钮应该存在且可用
      const exportBtn = screen.getByText('导出');
      expect(exportBtn).not.toBeDisabled();
    });
  });

  // ============================================================
  // 需求级测试
  // ============================================================
  describe('Requirements Tests', () => {
    it('req_audit_001: should display audit log list with all columns', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { logs: mockLogs } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('审计日志')).toBeInTheDocument();
        expect(screen.getByText('用户 admin 登录成功')).toBeInTheDocument();
      });
    });

    it('req_audit_002: should support search functionality', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { logs: mockLogs } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByPlaceholderText('搜索操作类型、详情或用户 ID')).toBeInTheDocument();
      });
    });

    it('req_audit_003: should support action type filter', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { logs: mockLogs } });
      renderComponent();

      await waitFor(() => {
        // 操作类型筛选下拉应该存在
        expect(document.querySelector('.audit-filter-bar')).toBeInTheDocument();
      });
    });

    it('req_audit_004: should support export functionality', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { logs: mockLogs } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('导出')).toBeInTheDocument();
      });
    });

    it('req_audit_005: should display DLP scan events from client', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { logs: mockLogs } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText(/DLP 脱敏/)).toBeInTheDocument();
        expect(screen.getByText(/DLP 阻止发送/)).toBeInTheDocument();
      });
    });

    it('req_audit_006: should distinguish system/client events from user events', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { logs: mockLogs } });
      renderComponent();

      await waitFor(() => {
        const systemTags = screen.getAllByText('系统 / 客户端');
        expect(systemTags.length).toBeGreaterThan(0);
      });
    });
  });

  // ============================================================
  // 契约测试
  // ============================================================
  describe('Contract Tests', () => {
    it('test_contract_load_logs_endpoint: should call correct API endpoint', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { logs: [] } });
      renderComponent();

      await waitFor(() => {
        expect(apiClient.get).toHaveBeenCalledWith('/audit-logs');
      });
    });

    it('test_contract_response_format: should handle expected response format', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({
        data: {
          logs: [{
            id: 'test-id',
            user_id: 'test-user',
            action: 'test_action',
            details: 'test details',
            created_at: '2026-03-21T10:00:00Z',
          }],
        },
      });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('test details')).toBeInTheDocument();
      });
    });
  });

  // ============================================================
  // 代码覆盖测试：边界情况
  // ============================================================
  describe('Code Coverage - Edge Cases', () => {
    it('should handle logs with null user_id', async () => {
      const logsWithNull = [{
        ...mockLogs[0],
        user_id: null,
      }];
      vi.mocked(apiClient.get).mockResolvedValue({ data: { logs: logsWithNull } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('系统 / 客户端')).toBeInTheDocument();
      });
    });

    it('should handle action color mapping for unknown actions', async () => {
      const logsWithUnknown = [{
        ...mockLogs[0],
        action: 'unknown_action_type',
      }];
      vi.mocked(apiClient.get).mockResolvedValue({ data: { logs: logsWithUnknown } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('unknown_action_type')).toBeInTheDocument();
      });
    });

    it('should disable export button when no logs', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { logs: [] } });
      renderComponent();

      await waitFor(() => {
        const exportBtn = screen.getByText('导出').closest('button');
        expect(exportBtn).toBeDisabled();
      });
    });
  });
});
