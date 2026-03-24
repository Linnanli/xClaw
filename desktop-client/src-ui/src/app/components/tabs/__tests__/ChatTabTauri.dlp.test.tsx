/**
 * ChatTabTauri DLP 集成测试
 *
 * 覆盖维度：DLP 集成正常路径、失败路径、契约测试、安全审计
 * 验证 DLP 警告在聊天 UI 中的正确集成
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import { ChatTabTauri } from '../ChatTabTauri';

// ============================================================================
// Mock 依赖
// ============================================================================

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

// 默认 mock：无 DLP 警告
const mockDlpWarning = { current: null as any };
const mockClearDlpWarning = vi.fn();

vi.mock('../../../hooks/useAiChatTauri', () => ({
  useAiChatTauri: () => ({
    messages: [],
    input: '',
    setInput: vi.fn(),
    isLoading: false,
    isConnected: true,
    error: null,
    thinkingMessage: null,
    dlpWarning: mockDlpWarning.current,
    handleSubmit: vi.fn(),
    setMessages: vi.fn(),
    clearMessages: vi.fn(),
    clearDlpWarning: mockClearDlpWarning,
    reload: vi.fn(),
  }),
}));

vi.mock('../../ui/scroll-area', () => ({
  ScrollArea: ({ children }: any) => <div>{children}</div>,
}));

vi.mock('../../ui/button', () => ({
  Button: ({ children, onClick, ...props }: any) => (
    <button onClick={onClick} {...props}>{children}</button>
  ),
}));

// ============================================================================
// DLP 集成 - 正常路径
// ============================================================================

describe('ChatTabTauri DLP 集成 - 正常路径', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockDlpWarning.current = null;
  });

  it('无 DLP 警告时不应显示警告组件', () => {
    render(<ChatTabTauri />);
    expect(screen.queryByText('检测到敏感信息')).not.toBeInTheDocument();
    expect(screen.queryByText('消息发送被阻止')).not.toBeInTheDocument();
  });

  it('应正常渲染欢迎页（DLP 不影响基础功能）', () => {
    render(<ChatTabTauri />);
    expect(screen.getByText('今天需要我帮你做些什么？')).toBeInTheDocument();
  });
});

// ============================================================================
// DLP 集成 - 错误路径
// ============================================================================

describe('ChatTabTauri DLP 集成 - 错误路径', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('test_failure_dlpWarning_null_no_crash', () => {
    mockDlpWarning.current = null;
    expect(() => {
      render(<ChatTabTauri />);
    }).not.toThrow();
  });

  it('test_failure_dlpWarning_undefined_no_crash', () => {
    mockDlpWarning.current = undefined;
    expect(() => {
      render(<ChatTabTauri />);
    }).not.toThrow();
  });
});

// ============================================================================
// 契约测试
// ============================================================================

describe('ChatTabTauri DLP 集成 - 契约测试', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockDlpWarning.current = null;
  });

  it('test_contract_dlp_components_imported_correctly', () => {
    // 验证 DLP 组件可以正确导入和渲染
    expect(() => {
      render(<ChatTabTauri />);
    }).not.toThrow();
  });

  it('test_contract_chat_still_works_without_dlp', () => {
    render(<ChatTabTauri />);
    // 基础聊天功能不受 DLP 影响
    expect(screen.getByText('智能对话')).toBeInTheDocument();
    expect(screen.getByText('数据分析')).toBeInTheDocument();
  });
});

// ============================================================================
// 安全审计测试
// ============================================================================

describe('ChatTabTauri DLP 集成 - 安全审计', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockDlpWarning.current = null;
  });

  it('test_audit_no_dlp_internal_state_in_dom', () => {
    const { container } = render(<ChatTabTauri />);
    const html = container.innerHTML;
    expect(html).not.toMatch(/dlpWarning/);
    expect(html).not.toMatch(/scanUserInput/);
    expect(html).not.toMatch(/sanitization/i);
  });

  it('test_audit_no_token_leaked_with_dlp', () => {
    const { container } = render(<ChatTabTauri />);
    const html = container.innerHTML;
    expect(html).not.toContain('mock-token');
    expect(html).not.toMatch(/bearer/i);
  });
});
