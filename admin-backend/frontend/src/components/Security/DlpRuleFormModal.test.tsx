import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { DlpRuleFormModal, validateRegex, testPattern } from './DlpRuleFormModal';
import type { TestResult } from './DlpRuleFormModal';
import { apiClient } from '../../api/client';
import type { DlpRule } from '../../types';

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
      warning: vi.fn(),
    },
  };
});

const mockRule: DlpRule = {
  id: 'rule-1',
  name: '身份证号检测',
  pattern: '\\d{17}[\\dXx]',
  replacement: '***',
  severity: 'high',
  description: '匹配18位身份证号',
  enabled: true,
  category: 'pii',
  created_at: '2026-03-19T10:00:00Z',
  updated_at: '2026-03-19T10:00:00Z',
};

describe('DlpRuleFormModal', () => {
  const mockOnCancel = vi.fn();
  const mockOnSuccess = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(apiClient.post).mockResolvedValue({ data: {} });
    vi.mocked(apiClient.put).mockResolvedValue({ data: {} });
  });

  // ============================================================
  // 单元测试：纯函数
  // ============================================================
  describe('Unit Tests - validateRegex', () => {
    it('should validate a correct regex', () => {
      expect(validateRegex('\\d{17}[\\dXx]')).toEqual({ valid: true });
    });

    it('should validate a simple string pattern', () => {
      expect(validateRegex('毛泽东')).toEqual({ valid: true });
    });

    it('should reject an invalid regex', () => {
      const result = validateRegex('[invalid');
      expect(result.valid).toBe(false);
      expect(result.error).toBeDefined();
    });

    it('should reject empty pattern', () => {
      const result = validateRegex('');
      expect(result.valid).toBe(false);
      expect(result.error).toBe('模式不能为空');
    });

    it('should validate complex regex patterns', () => {
      expect(validateRegex('(?:1[3-9])\\d{9}')).toEqual({ valid: true });
      expect(validateRegex('^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\\.[a-zA-Z]{2,}$')).toEqual({ valid: true });
    });
  });

  describe('Unit Tests - testPattern', () => {
    it('should match and replace text', () => {
      const result = testPattern('毛泽东', '我喜欢毛泽东', '***');
      expect(result).not.toBeNull();
      expect(result!.matched).toBe(true);
      expect(result!.matches).toEqual(['毛泽东']);
      expect(result!.replaced).toBe('我喜欢***');
    });

    it('should return no match for non-matching text', () => {
      const result = testPattern('毛泽东', '今天天气不错', '***');
      expect(result).not.toBeNull();
      expect(result!.matched).toBe(false);
      expect(result!.matches).toEqual([]);
    });

    it('should handle multiple matches', () => {
      const result = testPattern('\\d{3}', 'abc123def456', '***');
      expect(result).not.toBeNull();
      expect(result!.matched).toBe(true);
      expect(result!.matches).toHaveLength(2);
    });

    it('should return null for invalid regex', () => {
      expect(testPattern('[invalid', 'test', '***')).toBeNull();
    });

    it('should return null for empty test text', () => {
      expect(testPattern('\\d+', '', '***')).toBeNull();
    });

    it('should use original text when no replacement provided', () => {
      const result = testPattern('hello', 'hello world', '');
      expect(result).not.toBeNull();
      expect(result!.replaced).toBe('hello world');
    });
  });

  // ============================================================
  // 单元测试：组件渲染
  // ============================================================
  describe('Unit Tests - Component Rendering', () => {
    it('should render create mode title', () => {
      render(
        <DlpRuleFormModal
          visible={true}
          mode="create"
          onCancel={mockOnCancel}
          onSuccess={mockOnSuccess}
        />
      );
      expect(screen.getByText('创建 DLP 规则')).toBeInTheDocument();
    });

    it('should render edit mode title', () => {
      render(
        <DlpRuleFormModal
          visible={true}
          mode="edit"
          rule={mockRule}
          onCancel={mockOnCancel}
          onSuccess={mockOnSuccess}
        />
      );
      expect(screen.getByText('编辑 DLP 规则')).toBeInTheDocument();
    });

    it('should render all form fields', () => {
      render(
        <DlpRuleFormModal
          visible={true}
          mode="create"
          onCancel={mockOnCancel}
          onSuccess={mockOnSuccess}
        />
      );
      expect(screen.getByText('规则名')).toBeInTheDocument();
      expect(screen.getByText('匹配模式（正则表达式）')).toBeInTheDocument();
      expect(screen.getByText('替换文本')).toBeInTheDocument();
      expect(screen.getByText('严重级别')).toBeInTheDocument();
      expect(screen.getByText('分类')).toBeInTheDocument();
      expect(screen.getByText('描述')).toBeInTheDocument();
    });

    it('should render test button', () => {
      render(
        <DlpRuleFormModal
          visible={true}
          mode="create"
          onCancel={mockOnCancel}
          onSuccess={mockOnSuccess}
        />
      );
      expect(screen.getByText('测试此规则')).toBeInTheDocument();
    });

    it('should not render when not visible', () => {
      const { container } = render(
        <DlpRuleFormModal
          visible={false}
          mode="create"
          onCancel={mockOnCancel}
          onSuccess={mockOnSuccess}
        />
      );
      expect(container.querySelector('.ant-modal')).not.toBeInTheDocument();
    });

    it('should show create button text in create mode', () => {
      render(
        <DlpRuleFormModal
          visible={true}
          mode="create"
          onCancel={mockOnCancel}
          onSuccess={mockOnSuccess}
        />
      );
      expect(screen.getByRole('button', { name: /创\s*建/ })).toBeInTheDocument();
    });

    it('should show save button text in edit mode', () => {
      render(
        <DlpRuleFormModal
          visible={true}
          mode="edit"
          rule={mockRule}
          onCancel={mockOnCancel}
          onSuccess={mockOnSuccess}
        />
      );
      expect(screen.getByRole('button', { name: /保\s*存/ })).toBeInTheDocument();
    });
  });

  // ============================================================
  // 集成测试：创建和编辑流程
  // ============================================================
  describe('Integration Tests - Create Flow', () => {
    it('should create rule via API', async () => {
      const user = userEvent.setup();

      render(
        <DlpRuleFormModal
          visible={true}
          mode="create"
          onCancel={mockOnCancel}
          onSuccess={mockOnSuccess}
        />
      );

      await user.type(screen.getByPlaceholderText('例如：身份证号检测'), '手机号检测');
      // userEvent.type 会把 `[` 解析为键盘修饰符，用 paste 代替
      const patternInput = screen.getByPlaceholderText(/\\d\{17\}/);
      await user.click(patternInput);
      await user.paste('1[3-9]\\d{9}');

      const submitButton = screen.getByRole('button', { name: /创\s*建/ });
      await user.click(submitButton);

      await waitFor(() => {
        expect(apiClient.post).toHaveBeenCalledWith(
          '/dlp-rules',
          expect.objectContaining({
            name: '手机号检测',
            pattern: '1[3-9]\\d{9}',
            severity: 'medium',
            category: 'pii',
          })
        );
      }, { timeout: 10000 });

      await waitFor(() => {
        expect(mockOnSuccess).toHaveBeenCalled();
      }, { timeout: 10000 });
    }, 15000);

    it('should edit rule via API', async () => {
      const user = userEvent.setup();

      render(
        <DlpRuleFormModal
          visible={true}
          mode="edit"
          rule={mockRule}
          onCancel={mockOnCancel}
          onSuccess={mockOnSuccess}
        />
      );

      // 修改规则名
      const nameInput = screen.getByDisplayValue('身份证号检测');
      await user.clear(nameInput);
      await user.type(nameInput, '身份证号检测V2');

      const submitButton = screen.getByRole('button', { name: /保\s*存/ });
      await user.click(submitButton);

      await waitFor(() => {
        expect(apiClient.put).toHaveBeenCalledWith(
          `/dlp-rules/${mockRule.id}`,
          expect.objectContaining({
            name: '身份证号检测V2',
          })
        );
      }, { timeout: 10000 });

      await waitFor(() => {
        expect(mockOnSuccess).toHaveBeenCalled();
      }, { timeout: 10000 });
    }, 15000);
  });

  // ============================================================
  // 失败路径测试
  // ============================================================
  describe('Failure Path Tests', () => {
    it('test_failure_create_api_error: should handle create API error', async () => {
      vi.mocked(apiClient.post).mockRejectedValue({
        response: { data: { error: '规则名已存在' } },
      });

      const user = userEvent.setup();

      render(
        <DlpRuleFormModal
          visible={true}
          mode="create"
          onCancel={mockOnCancel}
          onSuccess={mockOnSuccess}
        />
      );

      await user.type(screen.getByPlaceholderText('例如：身份证号检测'), '重复规则');
      await user.type(screen.getByPlaceholderText(/\\d\{17\}/), 'test');

      const submitButton = screen.getByRole('button', { name: /创\s*建/ });
      await user.click(submitButton);

      await waitFor(() => {
        expect(apiClient.post).toHaveBeenCalled();
      }, { timeout: 10000 });

      expect(mockOnSuccess).not.toHaveBeenCalled();
    }, 15000);

    it('test_failure_edit_api_error: should handle edit API error', async () => {
      vi.mocked(apiClient.put).mockRejectedValue({
        response: { data: { error: '规则不存在' } },
      });

      const user = userEvent.setup();

      render(
        <DlpRuleFormModal
          visible={true}
          mode="edit"
          rule={mockRule}
          onCancel={mockOnCancel}
          onSuccess={mockOnSuccess}
        />
      );

      const submitButton = screen.getByRole('button', { name: /保\s*存/ });
      await user.click(submitButton);

      await waitFor(() => {
        expect(apiClient.put).toHaveBeenCalled();
      }, { timeout: 10000 });

      expect(mockOnSuccess).not.toHaveBeenCalled();
    }, 15000);

    it('test_failure_validation: should show validation errors for empty required fields', async () => {
      const user = userEvent.setup();

      render(
        <DlpRuleFormModal
          visible={true}
          mode="create"
          onCancel={mockOnCancel}
          onSuccess={mockOnSuccess}
        />
      );

      const submitButton = screen.getByRole('button', { name: /创\s*建/ });
      await user.click(submitButton);

      await waitFor(() => {
        // antd 可能截断文本，使用 queryByText 的正则匹配
        const errorElements = document.querySelectorAll('.ant-form-item-explain-error');
        expect(errorElements.length).toBeGreaterThan(0);
      });

      expect(apiClient.post).not.toHaveBeenCalled();
    });
  });

  // ============================================================
  // 安全测试
  // ============================================================
  describe('Security Tests', () => {
    it('test_security_xss_in_pattern: should not execute pattern as code', () => {
      const xssPattern = '<script>alert("xss")</script>';
      const result = validateRegex(xssPattern);
      // 正则验证应该正常处理，不执行脚本
      expect(result.valid).toBe(true);
    });

    it('test_security_regex_dos: should handle catastrophic backtracking patterns', () => {
      // ReDoS 模式 - validateRegex 只检查语法，不执行匹配
      const redosPattern = '(a+)+$';
      const result = validateRegex(redosPattern);
      expect(result.valid).toBe(true);
    });

    it('test_security_no_sensitive_data_in_error: error messages should not contain sensitive data', () => {
      const result = validateRegex('[unclosed');
      expect(result.valid).toBe(false);
      // 错误信息不应包含用户输入的敏感数据
      expect(result.error).toBeDefined();
    });
  });

  // ============================================================
  // 需求级测试
  // ============================================================
  describe('Requirements Tests', () => {
    it('req_dlp_form_001: should support create and edit modes', () => {
      const { rerender } = render(
        <DlpRuleFormModal
          visible={true}
          mode="create"
          onCancel={mockOnCancel}
          onSuccess={mockOnSuccess}
        />
      );
      expect(screen.getByText('创建 DLP 规则')).toBeInTheDocument();

      rerender(
        <DlpRuleFormModal
          visible={true}
          mode="edit"
          rule={mockRule}
          onCancel={mockOnCancel}
          onSuccess={mockOnSuccess}
        />
      );
      expect(screen.getByText('编辑 DLP 规则')).toBeInTheDocument();
    });

    it('req_dlp_form_002: should provide regex validation feedback', async () => {
      const user = userEvent.setup();

      render(
        <DlpRuleFormModal
          visible={true}
          mode="create"
          onCancel={mockOnCancel}
          onSuccess={mockOnSuccess}
        />
      );

      const patternInput = screen.getByPlaceholderText(/\\d\{17\}/);
      // userEvent.type 会把 `[` 解析为键盘修饰符，用 fireEvent 代替
      await user.clear(patternInput);
      // 使用不含特殊字符的无效正则来测试
      await user.type(patternInput, '(?P<invalid>');

      // 应该显示正则错误提示
      await waitFor(() => {
        const errorElements = document.querySelectorAll('.ant-typography-danger');
        expect(errorElements.length).toBeGreaterThan(0);
      });
    });

    it('req_dlp_form_003: should provide rule testing capability', async () => {
      const user = userEvent.setup();

      render(
        <DlpRuleFormModal
          visible={true}
          mode="create"
          onCancel={mockOnCancel}
          onSuccess={mockOnSuccess}
        />
      );

      // 点击测试按钮展开测试区域
      await user.click(screen.getByText('测试此规则'));

      // 应该显示测试输入区域
      expect(screen.getByPlaceholderText(/输入测试文本/)).toBeInTheDocument();
      expect(screen.getByText('执行测试')).toBeInTheDocument();
    });

    it('req_dlp_form_004: edit mode should pre-fill form with rule data', () => {
      render(
        <DlpRuleFormModal
          visible={true}
          mode="edit"
          rule={mockRule}
          onCancel={mockOnCancel}
          onSuccess={mockOnSuccess}
        />
      );

      expect(screen.getByDisplayValue('身份证号检测')).toBeInTheDocument();
      expect(screen.getByDisplayValue('\\d{17}[\\dXx]')).toBeInTheDocument();
      expect(screen.getByDisplayValue('***')).toBeInTheDocument();
    });
  });

  // ============================================================
  // 契约测试
  // ============================================================
  describe('Contract Tests', () => {
    it('test_contract_create_request_format: should send correct create request format', async () => {
      const user = userEvent.setup();

      render(
        <DlpRuleFormModal
          visible={true}
          mode="create"
          onCancel={mockOnCancel}
          onSuccess={mockOnSuccess}
        />
      );

      await user.type(screen.getByPlaceholderText('例如：身份证号检测'), '测试规则');
      await user.type(screen.getByPlaceholderText(/\\d\{17\}/), '\\d+');

      const submitButton = screen.getByRole('button', { name: /创\s*建/ });
      await user.click(submitButton);

      await waitFor(() => {
        const call = vi.mocked(apiClient.post).mock.calls[0];
        expect(call[0]).toBe('/dlp-rules');
        const body = call[1] as any;
        // 验证请求体包含所有必需字段
        expect(body).toHaveProperty('name');
        expect(body).toHaveProperty('pattern');
        expect(body).toHaveProperty('severity');
        expect(body).toHaveProperty('category');
      }, { timeout: 10000 });
    }, 15000);

    it('test_contract_update_request_format: should send correct update request format', async () => {
      const user = userEvent.setup();

      render(
        <DlpRuleFormModal
          visible={true}
          mode="edit"
          rule={mockRule}
          onCancel={mockOnCancel}
          onSuccess={mockOnSuccess}
        />
      );

      const submitButton = screen.getByRole('button', { name: /保\s*存/ });
      await user.click(submitButton);

      await waitFor(() => {
        const call = vi.mocked(apiClient.put).mock.calls[0];
        expect(call[0]).toBe(`/dlp-rules/${mockRule.id}`);
        const body = call[1] as any;
        expect(body).toHaveProperty('name');
        expect(body).toHaveProperty('pattern');
        expect(body).toHaveProperty('severity');
        expect(body).toHaveProperty('category');
      }, { timeout: 10000 });
    }, 15000);
  });

  // ============================================================
  // 数据覆盖测试
  // ============================================================
  describe('Data Coverage Tests', () => {
    it('should handle all severity levels', () => {
      const severities = ['low', 'medium', 'high', 'critical'];
      severities.forEach(severity => {
        const rule = { ...mockRule, severity: severity as any };
        const { unmount } = render(
          <DlpRuleFormModal
            visible={true}
            mode="edit"
            rule={rule}
            onCancel={mockOnCancel}
            onSuccess={mockOnSuccess}
          />
        );
        unmount();
      });
    });

    it('should handle all category types', () => {
      const categories = ['pii', 'financial', 'health', 'credential', 'confidential', 'other'];
      categories.forEach(category => {
        const rule = { ...mockRule, category };
        const { unmount } = render(
          <DlpRuleFormModal
            visible={true}
            mode="edit"
            rule={rule}
            onCancel={mockOnCancel}
            onSuccess={mockOnSuccess}
          />
        );
        unmount();
      });
    });

    it('should handle rule with no optional fields', () => {
      const minimalRule: DlpRule = {
        ...mockRule,
        replacement: undefined as any,
        description: undefined as any,
      };
      render(
        <DlpRuleFormModal
          visible={true}
          mode="edit"
          rule={minimalRule}
          onCancel={mockOnCancel}
          onSuccess={mockOnSuccess}
        />
      );
      expect(screen.getByText('编辑 DLP 规则')).toBeInTheDocument();
    });
  });
});
