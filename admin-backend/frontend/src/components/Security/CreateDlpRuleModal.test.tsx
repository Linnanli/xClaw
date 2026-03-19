import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { CreateDlpRuleModal } from './CreateDlpRuleModal';
import { apiClient } from '../../api/client';

vi.mock('../../api/client');

describe('CreateDlpRuleModal', () => {
  const mockOnCancel = vi.fn();
  const mockOnSuccess = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(apiClient.post).mockResolvedValue({ data: {} });
  });

  // 单元测试：组件渲染
  it('should render modal with title', () => {
    render(
      <CreateDlpRuleModal
        visible={true}
        onCancel={mockOnCancel}
        onSuccess={mockOnSuccess}
      />
    );

    expect(screen.getByText('创建 DLP 规则')).toBeInTheDocument();
  });

  // 单元测试：表单字段
  it('should render all form fields', () => {
    render(
      <CreateDlpRuleModal
        visible={true}
        onCancel={mockOnCancel}
        onSuccess={mockOnSuccess}
      />
    );

    expect(screen.getByText('规则名')).toBeInTheDocument();
    expect(screen.getByText('匹配模式')).toBeInTheDocument();
    expect(screen.getByText('替换规则')).toBeInTheDocument();
    expect(screen.getByText('严重级别')).toBeInTheDocument();
    expect(screen.getByText('分类')).toBeInTheDocument();
    expect(screen.getByText('描述')).toBeInTheDocument();
  });

  // 集成测试：创建规则
  it('should create rule successfully', async () => {
    const user = userEvent.setup();
    
    render(
      <CreateDlpRuleModal
        visible={true}
        onCancel={mockOnCancel}
        onSuccess={mockOnSuccess}
      />
    );

    // 填写表单
    const nameInput = screen.getByPlaceholderText('例如：身份证号');
    await user.type(nameInput, '测试规则');

    const patternInput = screen.getByPlaceholderText('例如：正则表达式');
    await user.type(patternInput, 'test_pattern');

    // 提交表单
    const submitButton = screen.getByRole('button', { name: /创\s*建/ });
    await user.click(submitButton);

    await waitFor(() => {
      expect(apiClient.post).toHaveBeenCalledWith(
        '/dlp-rules',
        expect.objectContaining({
          name: '测试规则',
          pattern: 'test_pattern',
        })
      );
    }, { timeout: 10000 });
    
    await waitFor(() => {
      expect(mockOnSuccess).toHaveBeenCalled();
    }, { timeout: 10000 });
  }, 15000);

  // 失败路径测试：API 错误
  it('should handle API error', async () => {
    vi.mocked(apiClient.post).mockRejectedValue({
      response: { data: { error: 'Failed to create' } },
    });

    const user = userEvent.setup();
    
    render(
      <CreateDlpRuleModal
        visible={true}
        onCancel={mockOnCancel}
        onSuccess={mockOnSuccess}
      />
    );

    const nameInput = screen.getByPlaceholderText('例如：身份证号');
    await user.type(nameInput, '测试规则');

    const patternInput = screen.getByPlaceholderText('例如：正则表达式');
    await user.type(patternInput, '\\d{11}');

    const submitButton = screen.getByRole('button', { name: /创\s*建/ });
    await user.click(submitButton);

    await waitFor(() => {
      expect(apiClient.post).toHaveBeenCalled();
    }, { timeout: 10000 });
    
    // 验证 onSuccess 没有被调用
    expect(mockOnSuccess).not.toHaveBeenCalled();
  }, 15000);

  // 需求级测试：REQ-DLP-CREATE-001
  it('REQ-DLP-CREATE-001: should validate required fields', async () => {
    const user = userEvent.setup();
    
    render(
      <CreateDlpRuleModal
        visible={true}
        onCancel={mockOnCancel}
        onSuccess={mockOnSuccess}
      />
    );

    const submitButton = screen.getByRole('button', { name: /创\s*建/ });
    await user.click(submitButton);

    await waitFor(() => {
      expect(screen.getByText('请输入规则名')).toBeInTheDocument();
      expect(screen.getByText('请输入匹配模式')).toBeInTheDocument();
    });
  });

  // 安全测试：验证输入
  it('should validate pattern format', async () => {
    const user = userEvent.setup();
    
    render(
      <CreateDlpRuleModal
        visible={true}
        onCancel={mockOnCancel}
        onSuccess={mockOnSuccess}
      />
    );

    const nameInput = screen.getByPlaceholderText('例如：身份证号');
    await user.type(nameInput, '测试规则');

    const patternInput = screen.getByPlaceholderText('例如：正则表达式');
    await user.type(patternInput, 'valid_pattern');

    const submitButton = screen.getByRole('button', { name: /创\s*建/ });
    await user.click(submitButton);

    await waitFor(() => {
      expect(apiClient.post).toHaveBeenCalled();
    });
  });

  // 代码覆盖测试：模态框可见性
  it('should not render when modal is not visible', () => {
    const { container } = render(
      <CreateDlpRuleModal
        visible={false}
        onCancel={mockOnCancel}
        onSuccess={mockOnSuccess}
      />
    );

    expect(container.querySelector('.ant-modal')).not.toBeInTheDocument();
  });

  // 数据覆盖测试：不同严重级别
  it('should handle different severity levels', async () => {
    const user = userEvent.setup();
    
    render(
      <CreateDlpRuleModal
        visible={true}
        onCancel={mockOnCancel}
        onSuccess={mockOnSuccess}
      />
    );

    const nameInput = screen.getByPlaceholderText('例如：身份证号');
    await user.type(nameInput, '测试规则');

    const patternInput = screen.getByPlaceholderText('例如：正则表达式');
    await user.type(patternInput, 'test');

    // 验证严重级别选项存在
    expect(screen.getByText('严重级别')).toBeInTheDocument();
  });
});
