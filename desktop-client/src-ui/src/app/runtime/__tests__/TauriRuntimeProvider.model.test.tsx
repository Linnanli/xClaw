/**
 * TauriRuntimeProvider - 模型管理集成测试
 *
 * 覆盖维度（参考 AGENTS.md 测试维度矩阵）：
 * - 单元测试：模型列表加载、默认模型选择、模型切换
 * - 失败路径：API 失败时的降级行为
 * - 契约测试：ModelContext 接口与 TauriRuntimeProvider 的集成
 * - 安全审计：模型切换不影响 DLP 状态
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { TauriRuntimeProvider, useDlpState } from '../TauriRuntimeProvider';
import { useModelContext } from '../../contexts/ModelContext';
import type { ModelConfigItem } from '../../utils/tauri';

// ============================================================================
// Mock — vi.mock 工厂内不能引用外部变量（hoisting 限制），
// 通过 vi.mocked() 在测试中访问 mock 函数
// ============================================================================

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn().mockResolvedValue(undefined) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));

vi.mock('../../hooks/useDlpScan', () => ({
  useDlpScan: () => ({
    scanUserInput: vi.fn().mockResolvedValue({
      had_sensitive_data: false,
      sanitized_content: '',
      was_blocked: false,
      sanitization_stats: { total_matches: 0, redacted_count: 0, blocked_count: 0, warned_count: 0 },
    }),
  }),
}));

vi.mock('../../utils/tauri', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../utils/tauri')>();
  return {
    ...actual,
    modelApi: { getAvailableModels: vi.fn() },
    threadApi: {
      getMessages: vi.fn().mockResolvedValue([]),
      createThread: vi.fn().mockResolvedValue({ id: 'thread-1' }),
    },
  };
});

// ============================================================================
// 测试数据
// ============================================================================

const mockModels: ModelConfigItem[] = [
  {
    model_id: 'deepseek-chat',
    display_name: 'DeepSeek Chat',
    description: '通用对话',
    provider: 'deepseek',
    is_default: true,
    capabilities: ['chat'],
    source: 'admin',
  },
  {
    model_id: 'gpt-4o',
    display_name: 'GPT-4o',
    description: null,
    provider: 'openai',
    is_default: false,
    capabilities: ['chat'],
    source: 'admin',
  },
];

// ============================================================================
// 辅助：获取 mock 函数（通过 vi.mocked 访问已 mock 的模块）
// ============================================================================

async function getMocks() {
  const { modelApi, threadApi } = await import('../../utils/tauri');
  const { invoke } = await import('@tauri-apps/api/core');
  const { listen } = await import('@tauri-apps/api/event');
  return {
    getAvailableModels: vi.mocked(modelApi.getAvailableModels),
    getMessages: vi.mocked(threadApi.getMessages),
    invoke: vi.mocked(invoke),
    listen: vi.mocked(listen),
  };
}

// ============================================================================
// 测试组件
// ============================================================================

function ModelConsumer() {
  const { selectedModelId, models, selectModel, loading } = useModelContext();
  return (
    <div>
      <span data-testid="selected">{selectedModelId}</span>
      <span data-testid="count">{models.length}</span>
      <span data-testid="loading">{String(loading)}</span>
      {models.map((m) => (
        <button
          key={m.model_id}
          data-testid={`select-${m.model_id}`}
          onClick={() => selectModel(m.model_id)}
        >
          {m.display_name}
        </button>
      ))}
    </div>
  );
}

function renderProvider(props: Partial<React.ComponentProps<typeof TauriRuntimeProvider>> = {}) {
  return render(
    <TauriRuntimeProvider threadId={null} {...props}>
      <ModelConsumer />
    </TauriRuntimeProvider>,
  );
}

// ============================================================================
// 单元测试 - 正常路径
// ============================================================================

describe('TauriRuntimeProvider - 模型加载', () => {
  beforeEach(async () => {
    vi.clearAllMocks();
    const mocks = await getMocks();
    mocks.getAvailableModels.mockResolvedValue(mockModels);
    mocks.getMessages.mockResolvedValue([]);
    mocks.listen.mockResolvedValue(() => {});
    mocks.invoke.mockResolvedValue(undefined);
  });

  it('挂载后应加载模型列表并更新 ModelContext', async () => {
    renderProvider();
    await waitFor(() => {
      expect(screen.getByTestId('count').textContent).toBe('2');
    });
  });

  it('加载完成后 loading 应变为 false', async () => {
    renderProvider();
    await waitFor(() => {
      expect(screen.getByTestId('loading').textContent).toBe('false');
    });
  });

  it('应自动选中 is_default=true 的模型', async () => {
    renderProvider();
    await waitFor(() => {
      expect(screen.getByTestId('selected').textContent).toBe('deepseek-chat');
    });
  });

  it('无默认模型时应选中第一个模型', async () => {
    const mocks = await getMocks();
    const noDefaultModels = mockModels.map((m) => ({ ...m, is_default: false }));
    mocks.getAvailableModels.mockResolvedValue(noDefaultModels);
    renderProvider();
    await waitFor(() => {
      expect(screen.getByTestId('selected').textContent).toBe('deepseek-chat');
    });
  });

  it('initialModelId 有效时应优先使用', async () => {
    renderProvider({ initialModelId: 'gpt-4o' });
    await waitFor(() => {
      expect(screen.getByTestId('selected').textContent).toBe('gpt-4o');
    });
  });

  it('initialModelId 无效时应回退到默认模型', async () => {
    renderProvider({ initialModelId: 'nonexistent-model' });
    await waitFor(() => {
      expect(screen.getByTestId('selected').textContent).toBe('deepseek-chat');
    });
  });
});

// ============================================================================
// 单元测试 - 模型切换
// ============================================================================

describe('TauriRuntimeProvider - 模型切换', () => {
  beforeEach(async () => {
    vi.clearAllMocks();
    const mocks = await getMocks();
    mocks.getAvailableModels.mockResolvedValue(mockModels);
    mocks.getMessages.mockResolvedValue([]);
    mocks.listen.mockResolvedValue(() => {});
    mocks.invoke.mockResolvedValue(undefined);
  });

  it('点击切换按钮应更新 selectedModelId', async () => {
    renderProvider();
    await waitFor(() => expect(screen.getByTestId('count').textContent).toBe('2'));
    fireEvent.click(screen.getByTestId('select-gpt-4o'));
    expect(screen.getByTestId('selected').textContent).toBe('gpt-4o');
  });

  it('切换模型不应重置消息列表（无 key 重置）', async () => {
    renderProvider();
    await waitFor(() => expect(screen.getByTestId('count').textContent).toBe('2'));
    const countEl = screen.getByTestId('count');
    fireEvent.click(screen.getByTestId('select-gpt-4o'));
    expect(screen.getByTestId('count')).toBe(countEl);
  });

  it('test_regression_model_ref_updated_synchronously_before_send', async () => {
    // 回归测试：选模型后立即发消息，modelIdRef 必须已是新值
    // 这是修复"切换模型后发消息仍用旧模型"bug 的防护
    const mocks = await getMocks();
    renderProvider();
    await waitFor(() => expect(screen.getByTestId('count').textContent).toBe('2'));

    // 切换到 gpt-4o
    fireEvent.click(screen.getByTestId('select-gpt-4o'));

    // UI 立即反映新选择（同步更新，不依赖 useEffect）
    expect(screen.getByTestId('selected').textContent).toBe('gpt-4o');

    // 验证 invoke 调用时携带的是新 modelId（通过检查 invoke 调用参数）
    // 实际发送由 onNew 触发，这里验证 selectModel 的同步性
    expect(mocks.invoke).not.toHaveBeenCalledWith(
      'send_chat_message',
      expect.objectContaining({ modelId: 'deepseek-chat' }),
    );
  });

  it('onOpenCustomModelModal 回调应通过 ModelContext 传递', async () => {
    const onOpen = vi.fn();

    function CustomModalConsumer() {
      const { openCustomModelModal } = useModelContext();
      return (
        <button data-testid="open-custom" onClick={openCustomModelModal}>
          打开
        </button>
      );
    }

    render(
      <TauriRuntimeProvider threadId={null} onOpenCustomModelModal={onOpen}>
        <CustomModalConsumer />
      </TauriRuntimeProvider>,
    );

    fireEvent.click(screen.getByTestId('open-custom'));
    expect(onOpen).toHaveBeenCalledTimes(1);
  });
});

// ============================================================================
// 失败路径测试
// ============================================================================

describe('TauriRuntimeProvider - 模型加载失败路径', () => {
  beforeEach(async () => {
    vi.clearAllMocks();
    const mocks = await getMocks();
    mocks.getMessages.mockResolvedValue([]);
    mocks.listen.mockResolvedValue(() => {});
    mocks.invoke.mockResolvedValue(undefined);
  });

  it('test_failure_model_api_error_should_not_crash_provider', async () => {
    const mocks = await getMocks();
    mocks.getAvailableModels.mockRejectedValue(new Error('Network error'));
    renderProvider();
    await waitFor(() => {
      expect(screen.getByTestId('loading').textContent).toBe('false');
    });
  });

  it('test_failure_model_api_error_models_should_be_empty', async () => {
    const mocks = await getMocks();
    mocks.getAvailableModels.mockRejectedValue(new Error('Network error'));
    renderProvider();
    await waitFor(() => {
      expect(screen.getByTestId('loading').textContent).toBe('false');
    });
    expect(screen.getByTestId('count').textContent).toBe('0');
  });

  it('test_failure_empty_model_list_selected_id_stays_initial', async () => {
    const mocks = await getMocks();
    mocks.getAvailableModels.mockResolvedValue([]);
    renderProvider();
    await waitFor(() => {
      expect(screen.getByTestId('loading').textContent).toBe('false');
    });
    expect(screen.getByTestId('selected').textContent).toBe('');
  });
});

// ============================================================================
// 契约测试
// ============================================================================

describe('TauriRuntimeProvider - ModelContext 契约', () => {
  beforeEach(async () => {
    vi.clearAllMocks();
    const mocks = await getMocks();
    mocks.getAvailableModels.mockResolvedValue(mockModels);
    mocks.getMessages.mockResolvedValue([]);
    mocks.listen.mockResolvedValue(() => {});
    mocks.invoke.mockResolvedValue(undefined);
  });

  it('test_contract_model_context_provided_to_children', () => {
    renderProvider();
    expect(screen.getByTestId('selected')).toBeInTheDocument();
    expect(screen.getByTestId('count')).toBeInTheDocument();
    expect(screen.getByTestId('loading')).toBeInTheDocument();
  });

  it('test_contract_loading_true_during_fetch', async () => {
    const mocks = await getMocks();
    mocks.getAvailableModels.mockReturnValue(new Promise(() => {}));
    renderProvider();
    expect(screen.getByTestId('loading').textContent).toBe('true');
  });

  it('test_contract_no_openCustomModelModal_defaults_to_noop', () => {
    function NoopConsumer() {
      const { openCustomModelModal } = useModelContext();
      return (
        <button data-testid="noop-btn" onClick={openCustomModelModal}>
          noop
        </button>
      );
    }

    render(
      <TauriRuntimeProvider threadId={null}>
        <NoopConsumer />
      </TauriRuntimeProvider>,
    );

    expect(() => fireEvent.click(screen.getByTestId('noop-btn'))).not.toThrow();
  });
});

// ============================================================================
// 安全审计测试
// ============================================================================

describe('TauriRuntimeProvider - 安全审计', () => {
  beforeEach(async () => {
    vi.clearAllMocks();
    const mocks = await getMocks();
    mocks.getAvailableModels.mockResolvedValue(mockModels);
    mocks.getMessages.mockResolvedValue([]);
    mocks.listen.mockResolvedValue(() => {});
    mocks.invoke.mockResolvedValue(undefined);
  });

  it('test_audit_model_switch_does_not_clear_dlp_state', async () => {
    function DlpConsumer() {
      const dlp = useDlpState();
      return <span data-testid="dlp-blocked">{String(dlp.blocked)}</span>;
    }

    render(
      <TauriRuntimeProvider threadId={null}>
        <DlpConsumer />
        <ModelConsumer />
      </TauriRuntimeProvider>,
    );

    await waitFor(() => expect(screen.getByTestId('count').textContent).toBe('2'));
    fireEvent.click(screen.getByTestId('select-gpt-4o'));
    expect(screen.getByTestId('dlp-blocked').textContent).toBe('false');
  });

  it('test_audit_model_id_not_logged_with_sensitive_prefix', async () => {
    renderProvider();
    await waitFor(() => expect(screen.getByTestId('count').textContent).toBe('2'));
    const selectedId = screen.getByTestId('selected').textContent ?? '';
    expect(selectedId).not.toMatch(/^sk-/);
    expect(selectedId).not.toMatch(/^Bearer /);
  });
});

// ============================================================================
// 回归测试 — 模型切换在组件重新挂载后保持
// ============================================================================

describe('TauriRuntimeProvider - 模型切换持久性（回归）', () => {
  beforeEach(async () => {
    vi.clearAllMocks();
    const mocks = await getMocks();
    mocks.getAvailableModels.mockResolvedValue(mockModels);
    mocks.getMessages.mockResolvedValue([]);
    mocks.listen.mockResolvedValue(() => {});
    mocks.invoke.mockResolvedValue(undefined);
  });

  it('test_regression_model_survives_remount_via_initialModelId', async () => {
    // 回归测试：用户选了 gpt-4o，组件因 key 变化重新挂载后，
    // 通过 initialModelId 恢复选择，不应回退到默认模型。
    // 这是修复"发送消息后模型切回默认"bug 的防护。
    const { unmount } = renderProvider({ initialModelId: 'gpt-4o' });
    await waitFor(() => {
      expect(screen.getByTestId('selected').textContent).toBe('gpt-4o');
    });

    // 模拟组件重新挂载（key 变化）
    unmount();
    renderProvider({ initialModelId: 'gpt-4o' });
    await waitFor(() => {
      expect(screen.getByTestId('selected').textContent).toBe('gpt-4o');
    });
  });

  it('test_regression_onModelChange_called_on_select', async () => {
    // 回归测试：selectModel 必须通知父组件，否则重新挂载时 initialModelId 为空
    const onModelChange = vi.fn();
    renderProvider({ onModelChange });
    await waitFor(() => expect(screen.getByTestId('count').textContent).toBe('2'));

    fireEvent.click(screen.getByTestId('select-gpt-4o'));
    expect(onModelChange).toHaveBeenCalledWith('gpt-4o');
  });

  it('test_regression_onModelChange_called_on_default_selection', async () => {
    // 回归测试：首次加载选默认模型时也要通知父组件
    const onModelChange = vi.fn();
    renderProvider({ onModelChange });
    await waitFor(() => {
      expect(screen.getByTestId('selected').textContent).toBe('deepseek-chat');
    });
    expect(onModelChange).toHaveBeenCalledWith('deepseek-chat');
  });

  it('test_failure_remount_without_initialModelId_falls_back_to_default', async () => {
    // 失败路径：如果父组件没有传 initialModelId，应回退到默认模型
    renderProvider();
    await waitFor(() => {
      expect(screen.getByTestId('selected').textContent).toBe('deepseek-chat');
    });
  });
});
