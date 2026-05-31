/**
 * ModelProvider 单元测试
 *
 * 覆盖维度（参考 AGENTS.md 测试维度矩阵）：
 * - 正常路径：挂载加载 / 默认模型 / initialModelId / 切换
 * - 失败路径：getAvailableModels 失败不崩溃、loading 会收尾、空列表
 * - 契约测试：通过既有 `useModelContext` 消费
 * - 副作用：selectModel 会调用 activateModel 并带上 api_base_url/api_key
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { ModelProvider } from '../ModelProvider';
import { useModelContext } from '../../../contexts/ModelContext';
import type { ModelConfigItem } from '@utils/tauri';

vi.mock('@utils/tauri', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@utils/tauri')>();
  return {
    ...actual,
    modelApi: {
      getAvailableModels: vi.fn(),
      activateModel: vi.fn().mockResolvedValue(undefined),
    },
  };
});

// ModelProvider 现在依赖 useEngineReady，让单测默认走"已就绪"路径；
// 就绪 gate 行为在 useEngineReady 自身的测试里覆盖。
vi.mock('../../../hooks/useEngineReady', () => ({
  useEngineReady: () => ({ ready: true, readyKey: 1 }),
}));

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
    api_base_url: 'https://api.openai.com/v1',
    api_key: 'sk-test',
  },
];

async function getMocks() {
  const { modelApi } = await import('@utils/tauri');
  return {
    getAvailableModels: vi.mocked(modelApi.getAvailableModels),
    activateModel: vi.mocked(modelApi.activateModel),
  };
}

function Consumer() {
  const { selectedModelId, models, selectModel, openCustomModelModal, loading } = useModelContext();
  return (
    <div>
      <span data-testid="selected">{selectedModelId}</span>
      <span data-testid="count">{models.length}</span>
      <span data-testid="loading">{String(loading)}</span>
      <button data-testid="open-custom" onClick={openCustomModelModal}>
        open
      </button>
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

function renderProvider(props: Partial<React.ComponentProps<typeof ModelProvider>> = {}) {
  return render(
    <ModelProvider {...props}>
      <Consumer />
    </ModelProvider>,
  );
}

describe('ModelProvider - 正常路径', () => {
  beforeEach(async () => {
    vi.clearAllMocks();
    const mocks = await getMocks();
    mocks.getAvailableModels.mockResolvedValue(mockModels);
    mocks.activateModel.mockResolvedValue(undefined);
  });

  it('挂载后加载模型并更新 context', async () => {
    renderProvider();
    await waitFor(() => expect(screen.getByTestId('count').textContent).toBe('2'));
  });

  it('loading 完成后为 false', async () => {
    renderProvider();
    await waitFor(() => expect(screen.getByTestId('loading').textContent).toBe('false'));
  });

  it('自动选中 is_default=true', async () => {
    renderProvider();
    await waitFor(() => expect(screen.getByTestId('selected').textContent).toBe('deepseek-chat'));
  });

  it('无默认模型时选中第一个', async () => {
    const mocks = await getMocks();
    mocks.getAvailableModels.mockResolvedValue(
      mockModels.map((m) => ({ ...m, is_default: false })),
    );
    renderProvider();
    await waitFor(() => expect(screen.getByTestId('selected').textContent).toBe('deepseek-chat'));
  });

  it('initialModelId 有效时优先使用', async () => {
    renderProvider({ initialModelId: 'gpt-4o' });
    await waitFor(() => expect(screen.getByTestId('selected').textContent).toBe('gpt-4o'));
  });

  it('initialModelId 无效时回退默认', async () => {
    renderProvider({ initialModelId: 'nonexistent' });
    await waitFor(() => expect(screen.getByTestId('selected').textContent).toBe('deepseek-chat'));
  });

  it('onModelChange 在回退时被调用', async () => {
    const onChange = vi.fn();
    renderProvider({ initialModelId: 'nonexistent', onModelChange: onChange });
    await waitFor(() => expect(onChange).toHaveBeenCalledWith('deepseek-chat'));
  });
});

describe('ModelProvider - 切换', () => {
  beforeEach(async () => {
    vi.clearAllMocks();
    const mocks = await getMocks();
    mocks.getAvailableModels.mockResolvedValue(mockModels);
    mocks.activateModel.mockResolvedValue(undefined);
  });

  it('点击切换后 selectedModelId 同步更新', async () => {
    renderProvider();
    await waitFor(() => expect(screen.getByTestId('count').textContent).toBe('2'));
    fireEvent.click(screen.getByTestId('select-gpt-4o'));
    expect(screen.getByTestId('selected').textContent).toBe('gpt-4o');
  });

  it('切换时触发 activateModel 并带 api_base_url/api_key', async () => {
    const mocks = await getMocks();
    renderProvider();
    await waitFor(() => expect(screen.getByTestId('count').textContent).toBe('2'));
    fireEvent.click(screen.getByTestId('select-gpt-4o'));
    expect(mocks.activateModel).toHaveBeenCalledWith({
      model_id: 'gpt-4o',
      api_base_url: 'https://api.openai.com/v1',
      api_key: 'sk-test',
    });
  });

  it('切换时触发 onModelChange', async () => {
    const onChange = vi.fn();
    renderProvider({ onModelChange: onChange });
    await waitFor(() => expect(screen.getByTestId('count').textContent).toBe('2'));
    onChange.mockClear(); // 忽略挂载时的初始回调
    fireEvent.click(screen.getByTestId('select-gpt-4o'));
    expect(onChange).toHaveBeenCalledWith('gpt-4o');
  });

  it('openCustomModelModal 触发回调', async () => {
    const onOpen = vi.fn();
    renderProvider({ onOpenCustomModelModal: onOpen });
    await waitFor(() => expect(screen.getByTestId('count').textContent).toBe('2'));
    fireEvent.click(screen.getByTestId('open-custom'));
    expect(onOpen).toHaveBeenCalledTimes(1);
  });
});

describe('ModelProvider - 失败路径', () => {
  beforeEach(async () => {
    vi.clearAllMocks();
    const mocks = await getMocks();
    mocks.activateModel.mockResolvedValue(undefined);
  });

  it('test_failure_get_models_rejects_does_not_crash', async () => {
    const mocks = await getMocks();
    mocks.getAvailableModels.mockRejectedValue(new Error('Network error'));
    renderProvider();
    await waitFor(() => expect(screen.getByTestId('loading').textContent).toBe('false'));
    expect(screen.getByTestId('count').textContent).toBe('0');
  });

  it('test_failure_empty_model_list_keeps_initial_id', async () => {
    const mocks = await getMocks();
    mocks.getAvailableModels.mockResolvedValue([]);
    renderProvider({ initialModelId: 'pre-set' });
    await waitFor(() => expect(screen.getByTestId('loading').textContent).toBe('false'));
    expect(screen.getByTestId('selected').textContent).toBe('pre-set');
  });

  it('test_failure_activate_rejects_does_not_revert_selection', async () => {
    const mocks = await getMocks();
    mocks.getAvailableModels.mockResolvedValue(mockModels);
    mocks.activateModel.mockRejectedValue(new Error('activate failed'));
    renderProvider();
    await waitFor(() => expect(screen.getByTestId('count').textContent).toBe('2'));
    fireEvent.click(screen.getByTestId('select-gpt-4o'));
    // UI 选择立即生效，后台激活失败不回滚
    expect(screen.getByTestId('selected').textContent).toBe('gpt-4o');
  });
});
