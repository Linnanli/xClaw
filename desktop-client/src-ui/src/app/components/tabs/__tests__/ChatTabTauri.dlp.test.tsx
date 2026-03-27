/**
 * ChatTabTauri DLP 集成测试
 *
 * 验证 DLP 状态通过 TauriRuntimeProvider → DlpContext → DlpBlockedDialogBridge 的集成
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

// DLP 状态可控 mock
const mockDlpState = {
  blocked: false,
  blockReason: null as string | null,
  clearBlock: vi.fn(),
  redactedStats: null,
  clearRedacted: vi.fn(),
  onBlocked: vi.fn(),
  onRedacted: vi.fn(),
};

vi.mock('../../../runtime/TauriRuntimeProvider', () => ({
  TauriRuntimeProvider: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="tauri-runtime-provider">{children}</div>
  ),
  useDlpState: () => mockDlpState,
}));

vi.mock('../../assistant-ui/thread', () => ({
  Thread: () => <div data-testid="thread-component" />,
}));

vi.mock('../../ai/DlpBlockedDialog', () => ({
  DlpBlockedDialog: ({ open, blockReason }: { open: boolean; blockReason?: string }) =>
    open ? (
      <div data-testid="dlp-blocked-dialog">
        {blockReason && <span data-testid="block-reason">{blockReason}</span>}
      </div>
    ) : null,
}));

vi.mock('../../ai/CustomModelModal', () => ({
  CustomModelModal: () => null,
}));

// ============================================================================
// DLP 集成 - 正常路径
// ============================================================================

describe('ChatTabTauri DLP 集成 - 正常路径', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockDlpState.blocked = false;
    mockDlpState.blockReason = null;
  });

  it('无 DLP 阻止时不应显示阻止对话框', () => {
    render(<ChatTabTauri />);
    expect(screen.queryByTestId('dlp-blocked-dialog')).not.toBeInTheDocument();
  });

  it('应正常渲染 Thread（DLP 不影响基础功能）', () => {
    render(<ChatTabTauri />);
    expect(screen.getByTestId('thread-component')).toBeInTheDocument();
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
    mockDlpState.blocked = false;
    mockDlpState.blockReason = null;
    expect(() => {
      render(<ChatTabTauri />);
    }).not.toThrow();
  });

  it('test_failure_dlpWarning_undefined_no_crash', () => {
    mockDlpState.blocked = false;
    (mockDlpState as any).blockReason = undefined;
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
    mockDlpState.blocked = false;
    mockDlpState.blockReason = null;
  });

  it('test_contract_dlp_components_imported_correctly', () => {
    expect(() => {
      render(<ChatTabTauri />);
    }).not.toThrow();
  });

  it('test_contract_chat_still_works_without_dlp', () => {
    render(<ChatTabTauri />);
    expect(screen.getByTestId('thread-component')).toBeInTheDocument();
  });

  it('DLP 阻止时应显示阻止对话框', () => {
    mockDlpState.blocked = true;
    mockDlpState.blockReason = '检测到 API 密钥';
    render(<ChatTabTauri />);
    expect(screen.getByTestId('dlp-blocked-dialog')).toBeInTheDocument();
  });
});

// ============================================================================
// 安全审计测试
// ============================================================================

describe('ChatTabTauri DLP 集成 - 安全审计', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockDlpState.blocked = false;
    mockDlpState.blockReason = null;
  });

  it('test_audit_no_dlp_internal_state_in_dom', () => {
    const { container } = render(<ChatTabTauri />);
    const html = container.innerHTML;
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
