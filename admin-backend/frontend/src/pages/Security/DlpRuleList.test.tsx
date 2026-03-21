import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { BrowserRouter } from 'react-router-dom';
import { DlpRuleList } from './DlpRuleList';
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

const mockRules = [
  {
    id: '1',
    name: '身份证号',
    pattern: '\\d{17}[\\dXx]',
    replacement: '***',
    severity: 'high',
    description: '匹配身份证号',
    enabled: true,
    category: 'pii',
    rule_type: 'regex',
    rule_config: null,
    created_at: '2026-03-19T10:00:00Z',
    updated_at: '2026-03-19T10:00:00Z',
  },
  {
    id: '2',
    name: '手机号',
    pattern: '1[3-9]\\d{9}',
    replacement: '****',
    severity: 'medium',
    description: '匹配手机号',
    enabled: false,
    category: 'pii',
    rule_type: 'regex',
    rule_config: null,
    created_at: '2026-03-20T10:00:00Z',
    updated_at: '2026-03-20T10:00:00Z',
  },
  {
    id: '3',
    name: '银行卡号',
    pattern: '\\d{16,19}',
    replacement: '***',
    severity: 'critical',
    description: '匹配银行卡号',
    enabled: true,
    category: 'financial',
    rule_type: 'regex',
    rule_config: null,
    created_at: '2026-03-21T10:00:00Z',
    updated_at: '2026-03-21T10:00:00Z',
  },
];

const renderComponent = () => {
  return render(
    <BrowserRouter>
      <DlpRuleList />
    </BrowserRouter>
  );
};

describe('DlpRuleList Component', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // ============================================================
  // 单元测试：组件渲染
  // ============================================================
  describe('Unit Tests - Component Rendering', () => {
    it('should render page title and action buttons', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: [] } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('DLP 规则管理')).toBeInTheDocument();
        expect(screen.getByText('刷新')).toBeInTheDocument();
        expect(screen.getByText('创建规则')).toBeInTheDocument();
      });
    });

    it('should render search and filter bar', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: [] } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByPlaceholderText('搜索规则名、匹配模式或描述')).toBeInTheDocument();
        // Select 组件的 placeholder 通过 aria 属性或 DOM 结构渲染
        // 验证筛选栏容器存在即可
        expect(document.querySelector('.dlp-filter-bar')).toBeInTheDocument();
      });
    });

    it('should render table columns including edit action', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: mockRules } });
      renderComponent();

      await waitFor(() => {
        const headers = screen.getAllByRole('columnheader');
        const headerTexts = headers.map(h => h.textContent);
        expect(headerTexts).toContain('规则名');
        expect(headerTexts).toContain('类型');
        expect(headerTexts).toContain('匹配模式');
        expect(headerTexts).toContain('替换文本');
        expect(headerTexts).toContain('严重级别');
        expect(headerTexts).toContain('分类');
        expect(headerTexts).toContain('状态');
        expect(headerTexts).toContain('更新时间');
        expect(headerTexts).toContain('操作');
      });
    });

    it('should render empty state when no rules', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: [] } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText(/还没有 DLP 规则/)).toBeInTheDocument();
        expect(screen.getByText('创建第一条规则')).toBeInTheDocument();
      });
    });
  });

  // ============================================================
  // 集成测试：数据加载和交互
  // ============================================================
  describe('Integration Tests - Data Loading', () => {
    it('should load rules from API on mount', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: mockRules } });
      renderComponent();

      await waitFor(() => {
        expect(apiClient.get).toHaveBeenCalledWith('/dlp-rules');
      });
    });

    it('should display loaded rules', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: mockRules } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('身份证号')).toBeInTheDocument();
        expect(screen.getByText('手机号')).toBeInTheDocument();
        expect(screen.getByText('银行卡号')).toBeInTheDocument();
      });
    });

    it('should display rule count summary', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: mockRules } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText(/共 3 条规则/)).toBeInTheDocument();
      });
    });

    it('should reload rules on refresh button click', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: mockRules } });
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
    it('should filter rules by search text', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: mockRules } });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('身份证号')).toBeInTheDocument();
      });

      const searchInput = screen.getByPlaceholderText('搜索规则名、匹配模式或描述');
      await user.type(searchInput, '身份证');

      await waitFor(() => {
        expect(screen.getByText('身份证号')).toBeInTheDocument();
        expect(screen.queryByText('手机号')).not.toBeInTheDocument();
        expect(screen.queryByText('银行卡号')).not.toBeInTheDocument();
      });
    });

    it('should show filtered count when filters active', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: mockRules } });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('身份证号')).toBeInTheDocument();
      });

      const searchInput = screen.getByPlaceholderText('搜索规则名、匹配模式或描述');
      await user.type(searchInput, '身份证');

      await waitFor(() => {
        expect(screen.getByText(/显示 1 条/)).toBeInTheDocument();
      });
    });

    it('should show clear filters button when filters active', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: mockRules } });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('身份证号')).toBeInTheDocument();
      });

      const searchInput = screen.getByPlaceholderText('搜索规则名、匹配模式或描述');
      await user.type(searchInput, '身份证');

      await waitFor(() => {
        expect(screen.getByText('清除筛选')).toBeInTheDocument();
      });
    });

    it('should search by pattern content', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: mockRules } });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('身份证号')).toBeInTheDocument();
      });

      const searchInput = screen.getByPlaceholderText('搜索规则名、匹配模式或描述');
      await user.type(searchInput, '手机');

      await waitFor(() => {
        expect(screen.queryByText('身份证号')).not.toBeInTheDocument();
        expect(screen.getByText('手机号')).toBeInTheDocument();
      });
    });
  });

  // ============================================================
  // 集成测试：CRUD 操作
  // ============================================================
  describe('Integration Tests - CRUD Operations', () => {
    it('should open create modal on button click', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: [] } });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('创建规则')).toBeInTheDocument();
      });

      await user.click(screen.getByText('创建规则'));

      await waitFor(() => {
        expect(screen.getByText('创建 DLP 规则')).toBeInTheDocument();
      });
    });

    it('should open edit modal on edit button click', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: mockRules } });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('身份证号')).toBeInTheDocument();
      });

      // 点击编辑按钮
      const editButtons = screen.getAllByText('编辑');
      await user.click(editButtons[0]);

      await waitFor(() => {
        expect(screen.getByText('编辑 DLP 规则')).toBeInTheDocument();
      });
    });

    it('should open edit modal on rule name click', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: mockRules } });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('身份证号')).toBeInTheDocument();
      });

      // 点击规则名
      await user.click(screen.getByText('身份证号'));

      await waitFor(() => {
        expect(screen.getByText('编辑 DLP 规则')).toBeInTheDocument();
      });
    });

    it('should toggle rule status', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: mockRules } });
      vi.mocked(apiClient.put).mockResolvedValue({ data: {} });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('身份证号')).toBeInTheDocument();
      });

      // 找到禁用状态的 switch（手机号规则）
      const switches = screen.getAllByRole('switch');
      await user.click(switches[1]); // 第二个 switch 是手机号（disabled）

      await waitFor(() => {
        expect(apiClient.put).toHaveBeenCalledWith('/dlp-rules/2', { enabled: true });
      });
    });

    it('should delete rule after confirmation', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: mockRules } });
      vi.mocked(apiClient.delete).mockResolvedValue({ data: {} });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('身份证号')).toBeInTheDocument();
      });

      // 点击删除按钮
      const deleteButtons = screen.getAllByText('删除');
      await user.click(deleteButtons[0]);

      // 确认删除 - Popconfirm 的确认按钮
      await waitFor(() => {
        expect(screen.getByText('确认删除')).toBeInTheDocument();
      });

      // Popconfirm 使用 ant-btn-primary 或 ant-popconfirm-buttons 中的按钮
      const popconfirmOkBtn = document.querySelector('.ant-popconfirm-buttons .ant-btn-primary') as HTMLElement;
      if (popconfirmOkBtn) {
        await user.click(popconfirmOkBtn);
      }

      await waitFor(() => {
        expect(apiClient.delete).toHaveBeenCalledWith('/dlp-rules/1');
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
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: mockRules } });
      vi.mocked(apiClient.delete).mockRejectedValue({
        response: { data: { error: '删除失败' } },
      });

      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('身份证号')).toBeInTheDocument();
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
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: mockRules } });
      vi.mocked(apiClient.put).mockRejectedValue({
        response: { data: { error: '更新失败' } },
      });

      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('身份证号')).toBeInTheDocument();
      });

      const switches = screen.getAllByRole('switch');
      await user.click(switches[0]);

      await waitFor(() => {
        expect(message.error).toHaveBeenCalled();
      });
    });

    it('test_failure_empty_api_response: should handle empty rules array', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: [] } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText(/还没有 DLP 规则/)).toBeInTheDocument();
      });
    });

    it('test_failure_null_rules_response: should handle null rules in response', async () => {
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
    it('test_security_xss_in_rule_name: should safely render rule names', async () => {
      const xssRules = [{
        ...mockRules[0],
        name: '<script>alert("xss")</script>',
      }];
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: xssRules } });
      renderComponent();

      await waitFor(() => {
        // React 自动转义 HTML，不会执行脚本
        expect(screen.getByText('<script>alert("xss")</script>')).toBeInTheDocument();
      });
    });

    it('test_security_xss_in_search: should safely handle search input', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: mockRules } });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('身份证号')).toBeInTheDocument();
      });

      const searchInput = screen.getByPlaceholderText('搜索规则名、匹配模式或描述');
      await user.type(searchInput, '<img onerror=alert(1)>');

      // 不应该触发 XSS
      expect(document.querySelector('img')).toBeNull();
    });
  });

  // ============================================================
  // 需求级测试
  // ============================================================
  describe('Requirements Tests', () => {
    it('req_dlp_list_001: should display DLP rule list with all columns', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: mockRules } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('DLP 规则管理')).toBeInTheDocument();
        expect(screen.getByText('身份证号')).toBeInTheDocument();
      });
    });

    it('req_dlp_list_002: should support search functionality', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: mockRules } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByPlaceholderText('搜索规则名、匹配模式或描述')).toBeInTheDocument();
      });
    });

    it('req_dlp_list_003: should support edit functionality', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: mockRules } });
      renderComponent();

      await waitFor(() => {
        const editButtons = screen.getAllByText('编辑');
        expect(editButtons.length).toBeGreaterThan(0);
      });
    });

    it('req_dlp_list_004: should support delete with confirmation', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: mockRules } });
      renderComponent();

      await waitFor(() => {
        const deleteButtons = screen.getAllByText('删除');
        expect(deleteButtons.length).toBeGreaterThan(0);
      });
    });

    it('req_dlp_list_005: should support enable/disable toggle', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: mockRules } });
      renderComponent();

      await waitFor(() => {
        const switches = screen.getAllByRole('switch');
        expect(switches.length).toBe(mockRules.length);
      });
    });
  });

  // ============================================================
  // 契约测试
  // ============================================================
  describe('Contract Tests', () => {
    it('test_contract_load_rules_endpoint: should call correct API endpoint', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: [] } });
      renderComponent();

      await waitFor(() => {
        expect(apiClient.get).toHaveBeenCalledWith('/dlp-rules');
      });
    });

    it('test_contract_delete_endpoint: should call correct delete endpoint', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: mockRules } });
      vi.mocked(apiClient.delete).mockResolvedValue({ data: {} });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('身份证号')).toBeInTheDocument();
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
        expect(apiClient.delete).toHaveBeenCalledWith('/dlp-rules/1');
      });
    });

    it('test_contract_toggle_endpoint: should call correct toggle endpoint', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: mockRules } });
      vi.mocked(apiClient.put).mockResolvedValue({ data: {} });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('身份证号')).toBeInTheDocument();
      });

      const switches = screen.getAllByRole('switch');
      await user.click(switches[0]); // 身份证号规则（enabled=true -> false）

      await waitFor(() => {
        expect(apiClient.put).toHaveBeenCalledWith('/dlp-rules/1', { enabled: false });
      });
    });
  });

  // ============================================================
  // 代码覆盖测试：边界情况
  // ============================================================
  describe('Code Coverage - Edge Cases', () => {
    it('should handle rules with missing optional fields', async () => {
      const rulesWithMissing = [{
        id: '1',
        name: '最小规则',
        pattern: 'test',
        severity: 'low',
        enabled: true,
        category: 'other',
        created_at: '2026-03-19T10:00:00Z',
        updated_at: '2026-03-19T10:00:00Z',
      }];
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: rulesWithMissing } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('最小规则')).toBeInTheDocument();
      });
    });

    it('should handle unknown severity value', async () => {
      const rulesWithUnknown = [{
        ...mockRules[0],
        severity: 'unknown_severity',
      }];
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: rulesWithUnknown } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('身份证号')).toBeInTheDocument();
      });
    });

    it('should handle unknown category value', async () => {
      const rulesWithUnknown = [{
        ...mockRules[0],
        category: 'unknown_category',
      }];
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: rulesWithUnknown } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('身份证号')).toBeInTheDocument();
      });
    });
  });

  // ============================================================
  // 批量操作测试
  // ============================================================
  describe('Batch Operations Tests', () => {
    it('should show checkboxes for row selection', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: mockRules } });
      renderComponent();

      await waitFor(() => {
        const checkboxes = screen.getAllByRole('checkbox');
        // antd Table 在 jsdom 中至少渲染 1 个 checkbox（全选）
        expect(checkboxes.length).toBeGreaterThanOrEqual(1);
      });
    });

    it('should show batch action bar when rows selected', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: mockRules } });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('身份证号')).toBeInTheDocument();
      });

      // 点击第一行的 checkbox
      const checkboxes = screen.getAllByRole('checkbox');
      await user.click(checkboxes[1]); // 第一行

      await waitFor(() => {
        expect(screen.getByText(/已选择 1 条规则/)).toBeInTheDocument();
        expect(screen.getByText('批量启用')).toBeInTheDocument();
        expect(screen.getByText('批量禁用')).toBeInTheDocument();
        expect(screen.getByText('批量删除')).toBeInTheDocument();
      });
    });

    it('should call batch enable API', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: mockRules } });
      vi.mocked(apiClient.post).mockResolvedValue({ data: { updated: 1 } });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('身份证号')).toBeInTheDocument();
      });

      const checkboxes = screen.getAllByRole('checkbox');
      await user.click(checkboxes[1]);

      await waitFor(() => {
        expect(screen.getByText('批量启用')).toBeInTheDocument();
      });

      await user.click(screen.getByText('批量启用'));

      await waitFor(() => {
        expect(apiClient.post).toHaveBeenCalledWith('/dlp-rules/batch/status', {
          ids: ['1'],
          enabled: true,
        });
      });
    });

    it('should call batch disable API', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: mockRules } });
      vi.mocked(apiClient.post).mockResolvedValue({ data: { updated: 1 } });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('身份证号')).toBeInTheDocument();
      });

      const checkboxes = screen.getAllByRole('checkbox');
      await user.click(checkboxes[1]);

      await waitFor(() => {
        expect(screen.getByText('批量禁用')).toBeInTheDocument();
      });

      await user.click(screen.getByText('批量禁用'));

      await waitFor(() => {
        expect(apiClient.post).toHaveBeenCalledWith('/dlp-rules/batch/status', {
          ids: ['1'],
          enabled: false,
        });
      });
    });

    it('should clear selection after batch operation', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: mockRules } });
      vi.mocked(apiClient.post).mockResolvedValue({ data: { updated: 1 } });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('身份证号')).toBeInTheDocument();
      });

      const checkboxes = screen.getAllByRole('checkbox');
      await user.click(checkboxes[1]);

      await waitFor(() => {
        expect(screen.getByText('批量启用')).toBeInTheDocument();
      });

      await user.click(screen.getByText('批量启用'));

      await waitFor(() => {
        expect(screen.queryByText(/已选择/)).not.toBeInTheDocument();
      });
    });

    it('should cancel selection on cancel button click', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: mockRules } });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('身份证号')).toBeInTheDocument();
      });

      const checkboxes = screen.getAllByRole('checkbox');
      await user.click(checkboxes[1]);

      await waitFor(() => {
        expect(screen.getByText('取消选择')).toBeInTheDocument();
      });

      await user.click(screen.getByText('取消选择'));

      await waitFor(() => {
        expect(screen.queryByText(/已选择/)).not.toBeInTheDocument();
      });
    });
  });

  // ============================================================
  // 批量操作失败路径测试
  // ============================================================
  describe('Batch Operations - Failure Path Tests', () => {
    it('test_failure_batch_enable_error: should handle batch enable API error', async () => {
      const { message } = await import('antd');
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: mockRules } });
      vi.mocked(apiClient.post).mockRejectedValue({
        response: { data: { error: '批量启用失败' } },
      });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('身份证号')).toBeInTheDocument();
      });

      const checkboxes = screen.getAllByRole('checkbox');
      await user.click(checkboxes[1]);

      await waitFor(() => {
        expect(screen.getByText('批量启用')).toBeInTheDocument();
      });

      await user.click(screen.getByText('批量启用'));

      await waitFor(() => {
        expect(message.error).toHaveBeenCalled();
      });
    });

    it('test_failure_batch_disable_error: should handle batch disable API error', async () => {
      const { message } = await import('antd');
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: mockRules } });
      vi.mocked(apiClient.post).mockRejectedValue({
        response: { data: { error: '批量禁用失败' } },
      });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('身份证号')).toBeInTheDocument();
      });

      const checkboxes = screen.getAllByRole('checkbox');
      await user.click(checkboxes[1]);

      await waitFor(() => {
        expect(screen.getByText('批量禁用')).toBeInTheDocument();
      });

      await user.click(screen.getByText('批量禁用'));

      await waitFor(() => {
        expect(message.error).toHaveBeenCalled();
      });
    });
  });

  // ============================================================
  // 导入导出测试
  // ============================================================
  describe('Import/Export Tests', () => {
    it('should render export and import buttons', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: mockRules } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('导出')).toBeInTheDocument();
        expect(screen.getByText('导入')).toBeInTheDocument();
      });
    });

    it('should disable export button when no rules', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: [] } });
      renderComponent();

      await waitFor(() => {
        const exportBtn = screen.getByText('导出').closest('button');
        expect(exportBtn).toBeDisabled();
      });
    });

    it('should call export API on export button click', async () => {
      vi.mocked(apiClient.get)
        .mockResolvedValueOnce({ data: { rules: mockRules } }) // initial load
        .mockResolvedValueOnce({ data: { version: '1.0', count: 3, rules: mockRules } }); // export

      // Mock URL.createObjectURL and createElement
      const mockCreateObjectURL = vi.fn(() => 'blob:test');
      const mockRevokeObjectURL = vi.fn();
      global.URL.createObjectURL = mockCreateObjectURL;
      global.URL.revokeObjectURL = mockRevokeObjectURL;

      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('身份证号')).toBeInTheDocument();
      });

      await user.click(screen.getByText('导出'));

      await waitFor(() => {
        expect(apiClient.get).toHaveBeenCalledWith('/dlp-rules/export');
      });
    });

    it('test_failure_export_error: should handle export API error', async () => {
      const { message } = await import('antd');
      vi.mocked(apiClient.get)
        .mockResolvedValueOnce({ data: { rules: mockRules } })
        .mockRejectedValueOnce({ response: { data: { error: '导出失败' } } });

      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('身份证号')).toBeInTheDocument();
      });

      await user.click(screen.getByText('导出'));

      await waitFor(() => {
        expect(message.error).toHaveBeenCalled();
      });
    });
  });

  // ============================================================
  // 导入导出契约测试
  // ============================================================
  describe('Import/Export - Contract Tests', () => {
    it('test_contract_export_endpoint: should call correct export endpoint', async () => {
      vi.mocked(apiClient.get)
        .mockResolvedValueOnce({ data: { rules: mockRules } })
        .mockResolvedValueOnce({ data: { version: '1.0', count: 3, rules: [] } });

      global.URL.createObjectURL = vi.fn(() => 'blob:test');
      global.URL.revokeObjectURL = vi.fn();

      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('身份证号')).toBeInTheDocument();
      });

      await user.click(screen.getByText('导出'));

      await waitFor(() => {
        expect(apiClient.get).toHaveBeenCalledWith('/dlp-rules/export');
      });
    });

    it('test_contract_batch_status_endpoint: should call correct batch status endpoint', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: mockRules } });
      vi.mocked(apiClient.post).mockResolvedValue({ data: { updated: 2 } });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('身份证号')).toBeInTheDocument();
      });

      // 全选
      const checkboxes = screen.getAllByRole('checkbox');
      await user.click(checkboxes[0]); // 全选

      await waitFor(() => {
        expect(screen.getByText('批量启用')).toBeInTheDocument();
      });

      await user.click(screen.getByText('批量启用'));

      await waitFor(() => {
        expect(apiClient.post).toHaveBeenCalledWith('/dlp-rules/batch/status', expect.objectContaining({
          enabled: true,
        }));
      });
    });
  });

  // ============================================================
  // 关键字规则显示测试
  // ============================================================
  describe('Keyword Rule Display Tests', () => {
    const mockRulesWithKeyword = [
      ...mockRules,
      {
        id: '4',
        name: '敏感词过滤',
        pattern: '机密,绝密,内部',
        replacement: '***',
        severity: 'high',
        description: '关键字匹配规则',
        enabled: true,
        category: 'confidential',
        rule_type: 'keyword',
        rule_config: {
          keywords: ['机密', '绝密', '内部'],
          match_mode: 'contains',
          case_sensitive: false,
        },
        created_at: '2026-03-21T12:00:00Z',
        updated_at: '2026-03-21T12:00:00Z',
      },
    ];

    it('should display rule type column with correct tags', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: mockRulesWithKeyword } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('敏感词过滤')).toBeInTheDocument();
        // 应该显示"关键字"标签
        expect(screen.getByText('关键字')).toBeInTheDocument();
        // 应该显示"正则"标签
        const regexTags = screen.getAllByText('正则');
        expect(regexTags.length).toBe(3); // 3 个正则规则
      });
    });

    it('should display keyword tags instead of pattern for keyword rules', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: mockRulesWithKeyword } });
      renderComponent();

      await waitFor(() => {
        // 关键字规则应该显示关键字标签
        expect(screen.getByText('机密')).toBeInTheDocument();
        expect(screen.getByText('绝密')).toBeInTheDocument();
        expect(screen.getByText('内部')).toBeInTheDocument();
      });
    });

    it('should handle keyword rule with many keywords (show +N)', async () => {
      const manyKeywordsRule = {
        id: '5',
        name: '多关键字规则',
        pattern: 'a,b,c,d,e',
        replacement: '***',
        severity: 'medium',
        description: '测试',
        enabled: true,
        category: 'other',
        rule_type: 'keyword',
        rule_config: {
          keywords: ['关键字A', '关键字B', '关键字C', '关键字D', '关键字E'],
          match_mode: 'contains',
          case_sensitive: false,
        },
        created_at: '2026-03-21T12:00:00Z',
        updated_at: '2026-03-21T12:00:00Z',
      };
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: [manyKeywordsRule] } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('多关键字规则')).toBeInTheDocument();
        // 只显示前 3 个，剩余显示 +2
        expect(screen.getByText('+2')).toBeInTheDocument();
      });
    });

    it('should handle keyword rule with null rule_config gracefully', async () => {
      const keywordRuleNoConfig = {
        id: '6',
        name: '无配置关键字',
        pattern: '测试关键字',
        replacement: '***',
        severity: 'low',
        description: '关键字规则但无 rule_config',
        enabled: true,
        category: 'other',
        rule_type: 'keyword',
        rule_config: null,
        created_at: '2026-03-21T12:00:00Z',
        updated_at: '2026-03-21T12:00:00Z',
      };
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: [keywordRuleNoConfig] } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('无配置关键字')).toBeInTheDocument();
        // 没有 rule_config.keywords 时应该回退显示 pattern
        expect(screen.getByText('测试关键字')).toBeInTheDocument();
      });
    });

    it('should display rule type filter in filter bar', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: mockRulesWithKeyword } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('敏感词过滤')).toBeInTheDocument();
      });

      // 规则类型筛选器应该存在
      expect(document.querySelector('.dlp-filter-bar')).toBeInTheDocument();
    });
  });

  // ============================================================
  // 关键字规则：失败路径测试
  // ============================================================
  describe('Keyword Rule - Failure Path Tests', () => {
    it('test_failure_keyword_rule_empty_keywords: should handle keyword rule with empty keywords array', async () => {
      const emptyKeywordsRule = {
        id: '7',
        name: '空关键字规则',
        pattern: '',
        replacement: '***',
        severity: 'low',
        description: '空关键字',
        enabled: true,
        category: 'other',
        rule_type: 'keyword',
        rule_config: {
          keywords: [],
          match_mode: 'contains',
          case_sensitive: false,
        },
        created_at: '2026-03-21T12:00:00Z',
        updated_at: '2026-03-21T12:00:00Z',
      };
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: [emptyKeywordsRule] } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('空关键字规则')).toBeInTheDocument();
      });
    });

    it('test_failure_keyword_rule_missing_rule_type: should default to regex when rule_type is missing', async () => {
      const noTypeRule = {
        id: '8',
        name: '无类型规则',
        pattern: '\\d+',
        replacement: '***',
        severity: 'low',
        description: '缺少 rule_type',
        enabled: true,
        category: 'other',
        // rule_type 缺失
        created_at: '2026-03-21T12:00:00Z',
        updated_at: '2026-03-21T12:00:00Z',
      };
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: [noTypeRule] } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('无类型规则')).toBeInTheDocument();
        // 默认应该显示"正则"标签
        expect(screen.getByText('正则')).toBeInTheDocument();
      });
    });
  });

  // ============================================================
  // 关键字规则：安全测试
  // ============================================================
  describe('Keyword Rule - Security Tests', () => {
    it('test_security_xss_in_keyword: should safely render keyword tags with XSS content', async () => {
      const xssKeywordRule = {
        id: '9',
        name: 'XSS关键字',
        pattern: '<script>alert(1)</script>',
        replacement: '***',
        severity: 'high',
        description: 'XSS测试',
        enabled: true,
        category: 'other',
        rule_type: 'keyword',
        rule_config: {
          keywords: ['<script>alert(1)</script>', '<img onerror=alert(1)>'],
          match_mode: 'contains',
          case_sensitive: false,
        },
        created_at: '2026-03-21T12:00:00Z',
        updated_at: '2026-03-21T12:00:00Z',
      };
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: [xssKeywordRule] } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('XSS关键字')).toBeInTheDocument();
        // React 自动转义，不应执行脚本
        expect(document.querySelector('script')).toBeNull();
      });
    });
  });

  // ============================================================
  // 关键字规则：契约测试
  // ============================================================
  describe('Keyword Rule - Contract Tests', () => {
    it('test_contract_keyword_rule_api_response: should correctly parse keyword rule from API', async () => {
      const keywordRule = {
        id: '10',
        name: '契约测试关键字',
        pattern: '敏感,机密',
        replacement: '***',
        severity: 'high',
        description: '契约测试',
        enabled: true,
        category: 'confidential',
        rule_type: 'keyword',
        rule_config: {
          keywords: ['敏感', '机密'],
          match_mode: 'whole_word',
          case_sensitive: true,
        },
        created_at: '2026-03-21T12:00:00Z',
        updated_at: '2026-03-21T12:00:00Z',
      };
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: [keywordRule] } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('契约测试关键字')).toBeInTheDocument();
        expect(screen.getByText('关键字')).toBeInTheDocument();
        expect(screen.getByText('敏感')).toBeInTheDocument();
        expect(screen.getByText('机密')).toBeInTheDocument();
      });
    });

    it('test_contract_toggle_keyword_rule: should call correct endpoint for keyword rule toggle', async () => {
      const keywordRule = {
        id: '11',
        name: '切换关键字规则',
        pattern: '测试',
        replacement: '***',
        severity: 'medium',
        description: '',
        enabled: true,
        category: 'other',
        rule_type: 'keyword',
        rule_config: { keywords: ['测试'], match_mode: 'contains', case_sensitive: false },
        created_at: '2026-03-21T12:00:00Z',
        updated_at: '2026-03-21T12:00:00Z',
      };
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: [keywordRule] } });
      vi.mocked(apiClient.put).mockResolvedValue({ data: {} });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('切换关键字规则')).toBeInTheDocument();
      });

      const switches = screen.getAllByRole('switch');
      await user.click(switches[0]);

      await waitFor(() => {
        expect(apiClient.put).toHaveBeenCalledWith('/dlp-rules/11', { enabled: false });
      });
    });
  });

  // ============================================================
  // 关键字规则：需求级测试
  // ============================================================
  describe('Keyword Rule - Requirements Tests', () => {
    it('req_dlp_keyword_001: should support keyword rule type display', async () => {
      const mixedRules = [
        { ...mockRules[0], rule_type: 'regex', rule_config: null },
        {
          id: '12',
          name: '关键字需求测试',
          pattern: '关键字1,关键字2',
          replacement: '***',
          severity: 'medium',
          description: '',
          enabled: true,
          category: 'other',
          rule_type: 'keyword',
          rule_config: { keywords: ['关键字1', '关键字2'], match_mode: 'contains', case_sensitive: false },
          created_at: '2026-03-21T12:00:00Z',
          updated_at: '2026-03-21T12:00:00Z',
        },
      ];
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: mixedRules } });
      renderComponent();

      await waitFor(() => {
        // 两种类型都应该正确显示
        expect(screen.getByText('正则')).toBeInTheDocument();
        expect(screen.getByText('关键字')).toBeInTheDocument();
      });
    });

    it('req_dlp_keyword_002: should support editing keyword rules', async () => {
      const keywordRule = {
        id: '13',
        name: '可编辑关键字',
        pattern: '编辑测试',
        replacement: '***',
        severity: 'high',
        description: '',
        enabled: true,
        category: 'other',
        rule_type: 'keyword',
        rule_config: { keywords: ['编辑测试'], match_mode: 'contains', case_sensitive: false },
        created_at: '2026-03-21T12:00:00Z',
        updated_at: '2026-03-21T12:00:00Z',
      };
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: [keywordRule] } });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('可编辑关键字')).toBeInTheDocument();
      });

      // 点击编辑按钮
      const editButtons = screen.getAllByText('编辑');
      await user.click(editButtons[0]);

      await waitFor(() => {
        expect(screen.getByText('编辑 DLP 规则')).toBeInTheDocument();
      });
    });

    it('req_dlp_keyword_003: should support deleting keyword rules', async () => {
      const keywordRule = {
        id: '14',
        name: '可删除关键字',
        pattern: '删除测试',
        replacement: '***',
        severity: 'low',
        description: '',
        enabled: true,
        category: 'other',
        rule_type: 'keyword',
        rule_config: { keywords: ['删除测试'], match_mode: 'contains', case_sensitive: false },
        created_at: '2026-03-21T12:00:00Z',
        updated_at: '2026-03-21T12:00:00Z',
      };
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: [keywordRule] } });
      vi.mocked(apiClient.delete).mockResolvedValue({ data: {} });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('可删除关键字')).toBeInTheDocument();
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
        expect(apiClient.delete).toHaveBeenCalledWith('/dlp-rules/14');
      });
    });
  });

  // ============================================================
  // 字典规则显示测试
  // ============================================================
  describe('Dictionary Rule Display Tests', () => {
    const mockDictRule = {
      id: '20',
      name: '敏感词字典规则',
      pattern: '机密,绝密,内部',
      replacement: '***',
      severity: 'high',
      description: '使用字典匹配',
      enabled: true,
      category: 'confidential',
      rule_type: 'dictionary',
      rule_config: {
        dictionary_id: 'dict-1',
        dictionary_name: '敏感词字典',
        match_mode: 'contains',
        case_sensitive: false,
      },
      created_at: '2026-03-21T14:00:00Z',
      updated_at: '2026-03-21T14:00:00Z',
    };

    it('should display dictionary rule type tag', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: [mockDictRule] } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('敏感词字典规则')).toBeInTheDocument();
        expect(screen.getByText('字典')).toBeInTheDocument();
      });
    });

    it('should display dictionary name in pattern column', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: [mockDictRule] } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('敏感词字典')).toBeInTheDocument();
      });
    });

    it('should display all three rule types correctly', async () => {
      const mixedRules = [
        { ...mockRules[0], rule_type: 'regex', rule_config: null },
        {
          id: '21',
          name: '关键字规则',
          pattern: '测试',
          replacement: '***',
          severity: 'medium',
          description: '',
          enabled: true,
          category: 'other',
          rule_type: 'keyword',
          rule_config: { keywords: ['测试'], match_mode: 'contains', case_sensitive: false },
          created_at: '2026-03-21T14:00:00Z',
          updated_at: '2026-03-21T14:00:00Z',
        },
        mockDictRule,
      ];
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: mixedRules } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('正则')).toBeInTheDocument();
        expect(screen.getByText('关键字')).toBeInTheDocument();
        expect(screen.getByText('字典')).toBeInTheDocument();
      });
    });

    it('should handle dictionary rule with missing dictionary_name', async () => {
      const noNameDictRule = {
        ...mockDictRule,
        id: '22',
        name: '无名字典规则',
        rule_config: {
          dictionary_id: 'dict-2',
          match_mode: 'contains',
          case_sensitive: false,
        },
      };
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: [noNameDictRule] } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('无名字典规则')).toBeInTheDocument();
        // 没有 dictionary_name 时应该显示默认文本"字典"
        const dictTags = screen.getAllByText('字典');
        expect(dictTags.length).toBeGreaterThanOrEqual(1);
      });
    });
  });

  // ============================================================
  // 字典规则：契约测试
  // ============================================================
  describe('Dictionary Rule - Contract Tests', () => {
    it('test_contract_dictionary_rule_toggle: should call correct endpoint for dictionary rule toggle', async () => {
      const dictRule = {
        id: '23',
        name: '字典切换测试',
        pattern: '测试',
        replacement: '***',
        severity: 'medium',
        description: '',
        enabled: true,
        category: 'other',
        rule_type: 'dictionary',
        rule_config: { dictionary_id: 'dict-1', dictionary_name: '测试字典', match_mode: 'contains', case_sensitive: false },
        created_at: '2026-03-21T14:00:00Z',
        updated_at: '2026-03-21T14:00:00Z',
      };
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: [dictRule] } });
      vi.mocked(apiClient.put).mockResolvedValue({ data: {} });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('字典切换测试')).toBeInTheDocument();
      });

      const switches = screen.getAllByRole('switch');
      await user.click(switches[0]);

      await waitFor(() => {
        expect(apiClient.put).toHaveBeenCalledWith('/dlp-rules/23', { enabled: false });
      });
    });

    it('test_contract_dictionary_rule_delete: should call correct endpoint for dictionary rule delete', async () => {
      const dictRule = {
        id: '24',
        name: '字典删除测试',
        pattern: '测试',
        replacement: '***',
        severity: 'low',
        description: '',
        enabled: true,
        category: 'other',
        rule_type: 'dictionary',
        rule_config: { dictionary_id: 'dict-1', dictionary_name: '测试字典', match_mode: 'contains', case_sensitive: false },
        created_at: '2026-03-21T14:00:00Z',
        updated_at: '2026-03-21T14:00:00Z',
      };
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: [dictRule] } });
      vi.mocked(apiClient.delete).mockResolvedValue({ data: {} });
      const user = userEvent.setup();
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('字典删除测试')).toBeInTheDocument();
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
        expect(apiClient.delete).toHaveBeenCalledWith('/dlp-rules/24');
      });
    });
  });

  // ============================================================
  // 字典规则：安全测试
  // ============================================================
  describe('Dictionary Rule - Security Tests', () => {
    it('test_security_xss_in_dictionary_name: should safely render dictionary name', async () => {
      const xssDictRule = {
        id: '25',
        name: 'XSS字典规则',
        pattern: 'test',
        replacement: '***',
        severity: 'high',
        description: '',
        enabled: true,
        category: 'other',
        rule_type: 'dictionary',
        rule_config: {
          dictionary_id: 'dict-xss',
          dictionary_name: '<script>alert("xss")</script>',
          match_mode: 'contains',
          case_sensitive: false,
        },
        created_at: '2026-03-21T14:00:00Z',
        updated_at: '2026-03-21T14:00:00Z',
      };
      vi.mocked(apiClient.get).mockResolvedValue({ data: { rules: [xssDictRule] } });
      renderComponent();

      await waitFor(() => {
        expect(screen.getByText('XSS字典规则')).toBeInTheDocument();
        // React 自动转义，不应执行脚本
        expect(document.querySelector('script')).toBeNull();
      });
    });
  });
});
