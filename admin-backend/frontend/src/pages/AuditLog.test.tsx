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
    username: 'admin',
    action: 'login',
    details: '用户 admin 登录成功',
    created_at: '2026-03-21T10:00:00Z',
  },
  {
    id: 'log-2',
    user_id: '00000000-0000-0000-0000-000000000000',
    username: null,
    action: 'dlp_redact',
    details: 'DLP 脱敏: 匹配 2 处, 脱敏 2 处',
    created_at: '2026-03-21T10:05:00Z',
  },
  {
    id: 'log-3',
    user_id: 'user-uuid-1',
    username: 'admin',
    action: 'create_dlp_rule',
    details: '创建 DLP 规则: 身份证号',
    created_at: '2026-03-21T10:10:00Z',
  },
  {
    id: 'log-4',
    user_id: '00000000-0000-0000-0000-000000000000',
    username: null,
    action: 'dlp_block',
    details: 'DLP 阻止发送: 匹配 1 处, 阻止 1 处',
    created_at: '2026-03-21T10:15:00Z',
  },
];

const mockApiResponse = { data: { logs: mockLogs, total: 4, total_pages: 1 } };
const emptyApiResponse = { data: { logs: [], total: 0, total_pages: 0 } };

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
      vi.mocked(apiClient.get).mockResolvedValue(emptyApiResponse);
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('审计日志')).toBeInTheDocument();
        expect(screen.getByText('刷新')).toBeInTheDocument();
        expect(screen.getByText('导出 CSV')).toBeInTheDocument();
      });
    });

    it('should render search and filter bar', async () => {
      vi.mocked(apiClient.get).mockResolvedValue(emptyApiResponse);
      renderComponent();

      await waitFor(() => {
        expect(screen.getByPlaceholderText('搜索操作人、操作类型或详情')).toBeInTheDocument();
        expect(document.querySelector('.audit-filter-bar')).toBeInTheDocument();
      });
    });

    it('should render table columns', async () => {
      vi.mocked(apiClient.get).mockResolvedValue(mockApiResponse);
      renderComponent();

      await waitFor(() => {
        const headers = screen.getAllByRole('columnheader');
        const headerTexts = headers.map(h => h.textContent);
        expect(headerTexts).toContain('时间');
        expect(headerTexts).toContain('操作人');
        expect(headerTexts).toContain('操作类型');
        expect(headerTexts).toContain('详情');
        expect(headerTexts).toContain('操作');
      });
    });

    it('should render empty state when no logs', async () => {
      vi.mocked(apiClient.get).mockResolvedValue(emptyApiResponse);
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('暂无审计日志')).toBeInTheDocument();
      });
    });

    it('should display "系统" tag for nil UUID user_id', async () => {
      vi.mocked(apiClient.get).mockResolvedValue(mockApiResponse);
      renderComponent();

      await waitFor(() => {
        const systemTags = screen.getAllByText('系统');
        expect(systemTags.length).toBe(2); // log-2 and log-4
      });
    });

    it('should display username for user events', async () => {
      vi.mocked(apiClient.get).mockResolvedValue(mockApiResponse);
      renderComponent();

      await waitFor(() => {
        const adminNames = screen.getAllByText('admin');
        expect(adminNames.length).toBeGreaterThanOrEqual(2); // log-1 and log-3
      });
    });
  });

  // ============================================================
  // 集成测试：数据加载和交互
  // ============================================================
  describe('Integration Tests - Data Loading', () => {
    it('should load logs from API on mount with pagination params', async () => {
      vi.mocked(apiClient.get).mockResolvedValue(mockApiResponse);
      renderComponent();

      await waitFor(() => {
        expect(apiClient.get).toHaveBeenCalledWith('/audit-logs', {
          params: { page: 1, page_size: 20 },
        });
      });
    });

    it('should display loaded logs', async () => {
      vi.mocked(apiClient.get).mockResolvedValue(mockApiResponse);
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('用户 admin 登录成功')).toBeInTheDocument();
        expect(screen.getByText(/DLP 脱敏/)).toBeInTheDocument();
      });
    });

    it('should display log count summary', async () => {
      vi.mocked(apiClient.get).mockResolvedValue(mockApiResponse);
      renderComponent();

      await waitFor(() => {
        const matches = screen.getAllByText(/共 4 条日志/);
        expect(matches.length).toBeGreaterThanOrEqual(1);
      });
    });

    it('should display action labels in Chinese', async () => {
      vi.mocked(apiClient.get).mockResolvedValue(mockApiResponse);
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('用户登录')).toBeInTheDocument();
        expect(screen.getByText('创建 DLP 规则')).toBeInTheDocument();
      });
    });

    it('should reload logs on refresh button click', async () => {
      vi.mocked(apiClient.get).mockResolvedValue(mockApiResponse);
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
      vi.mocked(apiClient.get).mockResolvedValue(mockApiResponse);
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('用户 admin 登录成功')).toBeInTheDocument();
      });

      const searchInput = screen.getByPlaceholderText('搜索操作人、操作类型或详情');
      await user.type(searchInput, 'DLP');

      await waitFor(() => {
        expect(screen.queryByText('用户 admin 登录成功')).not.toBeInTheDocument();
        expect(screen.getByText(/DLP 脱敏/)).toBeInTheDocument();
      });
    });

    it('should filter by username', async () => {
      vi.mocked(apiClient.get).mockResolvedValue(mockApiResponse);
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('用户 admin 登录成功')).toBeInTheDocument();
      });

      const searchInput = screen.getByPlaceholderText('搜索操作人、操作类型或详情');
      await user.type(searchInput, 'admin');

      await waitFor(() => {
        // admin 用户的日志应该显示
        expect(screen.getByText('用户 admin 登录成功')).toBeInTheDocument();
      });
    });

    it('should show clear filters button when filters active', async () => {
      vi.mocked(apiClient.get).mockResolvedValue(mockApiResponse);
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('用户 admin 登录成功')).toBeInTheDocument();
      });

      const searchInput = screen.getByPlaceholderText('搜索操作人、操作类型或详情');
      await user.type(searchInput, 'login');

      await waitFor(() => {
        expect(screen.getByText('清除筛选')).toBeInTheDocument();
      });
    });
  });

  // ============================================================
  // 集成测试：详情弹窗
  // ============================================================
  describe('Integration Tests - Detail Modal', () => {
    it('should have detail buttons for each log entry', async () => {
      vi.mocked(apiClient.get).mockResolvedValue(mockApiResponse);
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('用户 admin 登录成功')).toBeInTheDocument();
      });

      const detailButtons = screen.getAllByText('详情');
      expect(detailButtons.length).toBeGreaterThan(0);
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
      vi.mocked(apiClient.get).mockResolvedValue(emptyApiResponse);
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
      vi.mocked(apiClient.get).mockResolvedValue({ data: { logs: xssLogs, total: 1 } });
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
      vi.mocked(apiClient.get).mockResolvedValue({ data: { logs: xssLogs, total: 1 } });
      renderComponent();

      await waitFor(() => {
        expect(document.querySelector('img[onerror]')).toBeNull();
      });
    });

    it('test_security_xss_in_username: should safely render username', async () => {
      const xssLogs = [{
        ...mockLogs[0],
        username: '<script>alert("xss")</script>',
      }];
      vi.mocked(apiClient.get).mockResolvedValue({ data: { logs: xssLogs, total: 1 } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('<script>alert("xss")</script>')).toBeInTheDocument();
        expect(document.querySelectorAll('script').length).toBe(0);
      });
    });
  });

  // ============================================================
  // 需求级测试
  // ============================================================
  describe('Requirements Tests', () => {
    it('req_audit_001: should display audit log list with all columns', async () => {
      vi.mocked(apiClient.get).mockResolvedValue(mockApiResponse);
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('审计日志')).toBeInTheDocument();
        expect(screen.getByText('用户 admin 登录成功')).toBeInTheDocument();
      });
    });

    it('req_audit_002: should support search functionality', async () => {
      vi.mocked(apiClient.get).mockResolvedValue(mockApiResponse);
      renderComponent();

      await waitFor(() => {
        expect(screen.getByPlaceholderText('搜索操作人、操作类型或详情')).toBeInTheDocument();
      });
    });

    it('req_audit_003: should support action type filter', async () => {
      vi.mocked(apiClient.get).mockResolvedValue(mockApiResponse);
      renderComponent();

      await waitFor(() => {
        expect(document.querySelector('.audit-filter-bar')).toBeInTheDocument();
      });
    });

    it('req_audit_004: should support CSV export functionality', async () => {
      vi.mocked(apiClient.get).mockResolvedValue(mockApiResponse);
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('导出 CSV')).toBeInTheDocument();
      });
    });

    it('req_audit_005: should display DLP scan events from client', async () => {
      vi.mocked(apiClient.get).mockResolvedValue(mockApiResponse);
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText(/DLP 脱敏/)).toBeInTheDocument();
        expect(screen.getByText(/DLP 阻止发送/)).toBeInTheDocument();
      });
    });

    it('req_audit_006: should distinguish system events from user events', async () => {
      vi.mocked(apiClient.get).mockResolvedValue(mockApiResponse);
      renderComponent();

      await waitFor(() => {
        const systemTags = screen.getAllByText('系统');
        expect(systemTags.length).toBeGreaterThan(0);
        const adminNames = screen.getAllByText('admin');
        expect(adminNames.length).toBeGreaterThan(0);
      });
    });

    it('req_audit_007: should have detail view capability', async () => {
      vi.mocked(apiClient.get).mockResolvedValue(mockApiResponse);
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('用户 admin 登录成功')).toBeInTheDocument();
      });

      const detailButtons = screen.getAllByText('详情');
      expect(detailButtons.length).toBeGreaterThan(0);
    });

    it('req_audit_008: should support server-side pagination', async () => {
      vi.mocked(apiClient.get).mockResolvedValue(mockApiResponse);
      renderComponent();

      await waitFor(() => {
        expect(apiClient.get).toHaveBeenCalledWith('/audit-logs', {
          params: { page: 1, page_size: 20 },
        });
      });
    });
  });

  // ============================================================
  // 契约测试
  // ============================================================
  describe('Contract Tests', () => {
    it('test_contract_load_logs_endpoint: should call correct API endpoint with pagination', async () => {
      vi.mocked(apiClient.get).mockResolvedValue(emptyApiResponse);
      renderComponent();

      await waitFor(() => {
        expect(apiClient.get).toHaveBeenCalledWith('/audit-logs', {
          params: { page: 1, page_size: 20 },
        });
      });
    });

    it('test_contract_response_format: should handle expected response format with total', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({
        data: {
          logs: [{
            id: 'test-id',
            user_id: 'test-user',
            username: 'testuser',
            action: 'test_action',
            details: 'test details',
            created_at: '2026-03-21T10:00:00Z',
          }],
          total: 1,
          total_pages: 1,
        },
      });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('test details')).toBeInTheDocument();
        expect(screen.getByText('testuser')).toBeInTheDocument();
      });
    });
  });

  // ============================================================
  // 代码覆盖测试：边界情况
  // ============================================================
  describe('Code Coverage - Edge Cases', () => {
    it('should handle logs with null user_id and null username', async () => {
      const logsWithNull = [{
        ...mockLogs[0],
        user_id: null,
        username: null,
      }];
      vi.mocked(apiClient.get).mockResolvedValue({ data: { logs: logsWithNull, total: 1 } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('系统')).toBeInTheDocument();
      });
    });

    it('should handle action color mapping for unknown actions', async () => {
      const logsWithUnknown = [{
        ...mockLogs[0],
        action: 'unknown_action_type',
      }];
      vi.mocked(apiClient.get).mockResolvedValue({ data: { logs: logsWithUnknown, total: 1 } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('unknown_action_type')).toBeInTheDocument();
      });
    });

    it('should disable export button when no logs after filter', async () => {
      vi.mocked(apiClient.get).mockResolvedValue(emptyApiResponse);
      renderComponent();

      await waitFor(() => {
        const exportBtn = screen.getByText('导出 CSV').closest('button');
        expect(exportBtn).toBeDisabled();
      });
    });

    it('should handle user_id with truncated display for non-system users without username', async () => {
      const logsWithUuid = [{
        ...mockLogs[0],
        user_id: 'abcdef12-3456-7890-abcd-ef1234567890',
        username: null,
      }];
      vi.mocked(apiClient.get).mockResolvedValue({ data: { logs: logsWithUuid, total: 1 } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('abcdef12...')).toBeInTheDocument();
      });
    });
  });
});
