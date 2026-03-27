/**
 * ChatTabTauri 单元测试
 *
 * 组件架构：ChatTabTauri → TauriRuntimeProvider → Thread（assistant-ui）
 * 测试策略：mock TauriRuntimeProvider 和 Thread，验证组件集成契约
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

vi.mock('../../../utils/tokenManager', () => ({
  TokenManager: {
    getToken: vi.fn().mockResolvedValue('mock-token'),
  },
}));

vi.mock('../../../hooks/useModelConfig', () => ({
  useModelConfig: () => ({
    customModels: [],
    createModel: vi.fn(),
    updateModel: vi.fn(),
    deleteModel: vi.fn(),
    testConnection: vi.fn(),
  }),
}));

// Mock TauriRuntimeProvider：渲染 children，暴露 DlpContext
vi.mock('../../../runtime/TauriRuntimeProvider', () => ({
  TauriRuntimeProvider: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="tauri-runtime-provider">{children}</div>
  ),
  useDlpState: () => ({
    blocked: false,
    blockReason: null,
    clearBlock: vi.fn(),
    redactedStats: null,
    clearRedacted: vi.fn(),
    onBlocked: vi.fn(),
    onRedacted: vi.fn(),
  }),
}));

// Mock Thread 组件（assistant-ui，依赖 runtime context）
vi.mock('../../assistant-ui/thread', () => ({
  Thread: () => <div data-testid="thread-component">Thread</div>,
}));

// Mock DlpBlockedDialog
vi.mock('../../ai/DlpBlockedDialog', () => ({
  DlpBlockedDialog: ({ open }: { open: boolean }) =>
    open ? <div data-testid="dlp-blocked-dialog" /> : null,
}));

// Mock CustomModelModal
vi.mock('../../ai/CustomModelModal', () => ({
  CustomModelModal: ({ open }: { open: boolean }) =>
    open ? <div data-testid="custom-model-modal" /> : null,
}));

// ============================================================================
// 测试
// ============================================================================

describe('ChatTabTauri', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // ===== 正常路径 =====

  it('应渲染 TauriRuntimeProvider 和 Thread', () => {
    render(<ChatTabTauri />);
    expect(screen.getByTestId('tauri-runtime-provider')).toBeInTheDocument();
    expect(screen.getByTestId('thread-component')).toBeInTheDocument();
  });

  it('默认不显示 DLP 阻止对话框', () => {
    render(<ChatTabTauri />);
    expect(screen.queryByTestId('dlp-blocked-dialog')).not.toBeInTheDocument();
  });

  it('默认不显示自定义模型弹窗', () => {
    render(<ChatTabTauri />);
    expect(screen.queryByTestId('custom-model-modal')).not.toBeInTheDocument();
  });

  // ===== 错误路径 =====

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

  it('test_contract_uses_tauri_runtime_provider', () => {
    render(<ChatTabTauri />);
    // 验证组件使用了 TauriRuntimeProvider（而非旧的 useAiChatTauri）
    expect(screen.getByTestId('tauri-runtime-provider')).toBeInTheDocument();
  });

  // ===== 安全审计 =====

  it('test_audit_no_token_in_rendered_output', () => {
    const { container } = render(<ChatTabTauri />);
    expect(container.innerHTML).not.toContain('mock-token');
    expect(container.innerHTML).not.toMatch(/bearer/i);
    expect(container.innerHTML).not.toMatch(/api[_-]?key/i);
  });

  it('test_audit_no_sensitive_data_in_dom', () => {
    const { container } = render(<ChatTabTauri />);
    expect(container.innerHTML).not.toMatch(/password/i);
    expect(container.innerHTML).not.toMatch(/secret/i);
  });
});
