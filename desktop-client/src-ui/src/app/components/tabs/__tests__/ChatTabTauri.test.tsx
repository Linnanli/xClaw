/**
 * ChatTabTauri 单元测试
 *
 * 覆盖维度：正常路径、错误路径、契约测试、安全审计
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ChatTabTauri } from '../ChatTabTauri';

// Mock 依赖
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue(null),
}));

vi.mock('../../../utils/tauri', () => ({
  threadApi: {
    getThreads: vi.fn().mockResolvedValue([]),
    getMessages: vi.fn().mockResolvedValue([]),
    createThread: vi.fn().mockResolvedValue({ id: 'new-thread', title: '新对话' }),
  },
}));

vi.mock('../../../utils/tokenManager', () => ({
  TokenManager: {
    getToken: vi.fn().mockResolvedValue('mock-token'),
  },
}));

vi.mock('../../../hooks/useAiChatTauri', () => ({
  useAiChatTauri: () => ({
    messages: [],
    input: '',
    setInput: vi.fn(),
    isLoading: false,
    isConnected: true,
    error: null,
    thinkingMessage: null,
    handleSubmit: vi.fn(),
    setMessages: vi.fn(),
    clearMessages: vi.fn(),
  }),
}));

vi.mock('../../common/MessageActions', () => ({
  MessageActions: () => <div data-testid="message-actions" />,
}));

vi.mock('../../common/MessageEditor', () => ({
  MessageEditor: () => <div data-testid="message-editor" />,
}));

vi.mock('../../common/DeleteConfirmDialog', () => ({
  DeleteConfirmDialog: ({ isOpen }: any) =>
    isOpen ? <div data-testid="delete-dialog" /> : null,
}));

vi.mock('../../ui/scroll-area', () => ({
  ScrollArea: ({ children }: any) => <div>{children}</div>,
}));

vi.mock('../../ui/button', () => ({
  Button: ({ children, onClick, ...props }: any) => (
    <button onClick={onClick} {...props}>{children}</button>
  ),
}));

describe('ChatTabTauri', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // ===== 正常路径测试 =====

  it('无消息时应显示欢迎页', () => {
    render(<ChatTabTauri />);
    expect(screen.getByText('今天需要我帮你做些什么？')).toBeInTheDocument();
    expect(screen.getByText('你的专属 AI 团队已就绪')).toBeInTheDocument();
  });

  it('应显示快捷操作按钮', () => {
    render(<ChatTabTauri />);
    expect(screen.getByText('智能对话')).toBeInTheDocument();
    expect(screen.getByText('数据分析')).toBeInTheDocument();
    expect(screen.getByText('文档处理')).toBeInTheDocument();
    expect(screen.getByText('技能助手')).toBeInTheDocument();
  });

  it('应显示输入框和占位文本', () => {
    render(<ChatTabTauri />);
    expect(
      screen.getByPlaceholderText('请输入您的需求，或上传文件，AI 将为您解决问题。'),
    ).toBeInTheDocument();
  });

  it('应显示模型选择器', () => {
    render(<ChatTabTauri />);
    expect(screen.getByText('GPT-4o')).toBeInTheDocument();
  });

  it('应显示附件和图片按钮', () => {
    render(<ChatTabTauri />);
    expect(screen.getByLabelText('附件')).toBeInTheDocument();
    expect(screen.getByLabelText('图片')).toBeInTheDocument();
  });

  it('应显示发送按钮', () => {
    render(<ChatTabTauri />);
    expect(screen.getByLabelText('发送')).toBeInTheDocument();
  });

  it('应显示 @ 符号', () => {
    render(<ChatTabTauri />);
    expect(screen.getByText('@')).toBeInTheDocument();
  });

  // ===== 错误路径测试 =====

  it('test_failure_renders_without_thread_id', () => {
    expect(() => {
      render(<ChatTabTauri selectedThreadId={null} />);
    }).not.toThrow();
  });

  it('test_failure_renders_without_any_props', () => {
    expect(() => {
      render(<ChatTabTauri />);
    }).not.toThrow();
  });

  // ===== 契约测试 =====

  it('test_contract_accepts_optional_selectedThreadId', () => {
    expect(() => {
      render(<ChatTabTauri selectedThreadId="thread-123" />);
    }).not.toThrow();
  });

  it('test_contract_accepts_optional_onThreadSelect', () => {
    const onThreadSelect = vi.fn();
    expect(() => {
      render(<ChatTabTauri onThreadSelect={onThreadSelect} />);
    }).not.toThrow();
  });

  // ===== 安全审计测试 =====

  it('test_audit_no_token_in_rendered_output', () => {
    const { container } = render(<ChatTabTauri />);
    const html = container.innerHTML;
    expect(html).not.toContain('mock-token');
    expect(html).not.toMatch(/bearer/i);
    expect(html).not.toMatch(/api[_-]?key/i);
  });

  it('test_audit_no_sensitive_data_in_welcome_page', () => {
    const { container } = render(<ChatTabTauri />);
    const html = container.innerHTML;
    expect(html).not.toMatch(/password/i);
    expect(html).not.toMatch(/secret/i);
    expect(html).not.toMatch(/session/i);
  });
});
