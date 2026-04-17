import { describe, it, expect, beforeEach, vi } from 'vitest';
import { renderHook, waitFor, act } from '@testing-library/react';
import { useModelConfig } from '../useModelConfig';

vi.mock('../../utils/tauri', () => ({
  modelApi: {
    getAvailableModels: vi.fn(),
    getCustomModels: vi.fn(),
    createCustomModel: vi.fn(),
    updateCustomModel: vi.fn(),
    deleteCustomModel: vi.fn(),
    testConnection: vi.fn(),
  },
}));

import { modelApi } from '../../utils/tauri';

const mocks = {
  getAvailableModels: vi.mocked(modelApi.getAvailableModels),
  getCustomModels: vi.mocked(modelApi.getCustomModels),
  createCustomModel: vi.mocked(modelApi.createCustomModel),
  updateCustomModel: vi.mocked(modelApi.updateCustomModel),
  deleteCustomModel: vi.mocked(modelApi.deleteCustomModel),
  testConnection: vi.mocked(modelApi.testConnection),
};

describe('useModelConfig', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.getCustomModels.mockResolvedValue([]);
    mocks.getAvailableModels.mockResolvedValue([]);
    mocks.createCustomModel.mockResolvedValue({
      model_id: 'm1',
      display_name: 'M1',
      description: null,
      provider: 'openai',
      api_base_url: 'https://example.com/v1',
      api_key: 'sk-1',
      capabilities: [],
      extra_config: {},
      created_at: '2026-01-01T00:00:00Z',
      updated_at: '2026-01-01T00:00:00Z',
    });
    mocks.updateCustomModel.mockResolvedValue({
      model_id: 'm1',
      display_name: 'M1-updated',
      description: null,
      provider: 'openai',
      api_base_url: 'https://example.com/v1',
      api_key: 'sk-1',
      capabilities: [],
      extra_config: {},
      created_at: '2026-01-01T00:00:00Z',
      updated_at: '2026-01-02T00:00:00Z',
    });
    mocks.deleteCustomModel.mockResolvedValue();
    mocks.testConnection.mockResolvedValue({ success: true, message: 'ok' });
  });

  it('mount 时仅加载自定义模型，不调用 getAvailableModels', async () => {
    renderHook(() => useModelConfig());

    await waitFor(() => {
      expect(mocks.getCustomModels).toHaveBeenCalledTimes(1);
    });

    expect(mocks.getAvailableModels).not.toHaveBeenCalled();
  });

  it('refresh 显式调用时加载 available + custom 两类模型', async () => {
    const { result } = renderHook(() => useModelConfig());

    await waitFor(() => {
      expect(mocks.getCustomModels).toHaveBeenCalledTimes(1);
    });

    await act(async () => {
      await result.current.refresh();
    });

    expect(mocks.getAvailableModels).toHaveBeenCalledTimes(1);
    expect(mocks.getCustomModels).toHaveBeenCalledTimes(2);
  });

  it('create/update/delete 后仅重新加载自定义模型', async () => {
    const { result } = renderHook(() => useModelConfig());

    await waitFor(() => {
      expect(mocks.getCustomModels).toHaveBeenCalledTimes(1);
    });

    await act(async () => {
      await result.current.createModel({
        model_id: 'm1',
        display_name: 'M1',
        provider: 'openai',
        api_base_url: 'https://example.com/v1',
        api_key: 'sk-1',
      });
      await result.current.updateModel({
        model_id: 'm1',
        display_name: 'M1-updated',
      });
      await result.current.deleteModel('m1');
    });

    expect(mocks.getCustomModels).toHaveBeenCalledTimes(4);
    expect(mocks.getAvailableModels).not.toHaveBeenCalled();
  });
});
