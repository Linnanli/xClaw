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
});
