/**
 * useModelConfig - 模型配置管理 Hook
 *
 * 管理模型列表状态、加载、选择、CRUD 操作。
 * 数据来源：Admin Backend 下发 + 本地自定义模型。
 */

import { useState, useEffect, useCallback } from 'react';
import { modelApi, type ModelConfigItem, type CustomModelItem } from '../utils/tauri';

export interface UseModelConfigReturn {
  /** 所有可用模型（后台 + 自定义） */
  models: ModelConfigItem[];
  /** 本地自定义模型（含 API 配置） */
  customModels: CustomModelItem[];
  /** 当前选中的模型 ID */
  selectedModelId: string;
  /** 选择模型 */
  selectModel: (modelId: string) => void;
  /** 加载中 */
  loading: boolean;
  /** 错误信息 */
  error: string | null;
  /** 刷新模型列表 */
  refresh: () => Promise<void>;
  /** 创建自定义模型 */
  createModel: (params: CreateModelParams) => Promise<CustomModelItem>;
  /** 更新自定义模型 */
  updateModel: (params: UpdateModelParams) => Promise<CustomModelItem>;
  /** 删除自定义模型 */
  deleteModel: (modelId: string) => Promise<void>;
  /** 测试模型连接 */
  testConnection: (params: TestConnectionParams) => Promise<TestResult>;
}

export interface CreateModelParams {
  model_id: string;
  display_name: string;
  description?: string;
  provider: string;
  api_base_url: string;
  api_key: string;
}

export interface UpdateModelParams {
  model_id: string;
  display_name?: string;
  description?: string;
  provider?: string;
  api_base_url?: string;
  api_key?: string;
}

export interface TestConnectionParams {
  api_base_url: string;
  api_key: string;
  model_id: string;
}

export interface TestResult {
  success: boolean;
  message: string;
}

export function useModelConfig(): UseModelConfigReturn {
  const [models, setModels] = useState<ModelConfigItem[]>([]);
  const [customModels, setCustomModels] = useState<CustomModelItem[]>([]);
  const [selectedModelId, setSelectedModelId] = useState<string>('');
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const [allModels, customs] = await Promise.all([
        modelApi.getAvailableModels(),
        modelApi.getCustomModels(),
      ]);
      setModels(allModels);
      setCustomModels(customs);

      // 如果没有选中模型，选择默认模型
      if (!selectedModelId || !allModels.some((m) => m.model_id === selectedModelId)) {
        const defaultModel = allModels.find((m) => m.is_default) || allModels[0];
        if (defaultModel) setSelectedModelId(defaultModel.model_id);
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(msg);
      console.error('加载模型列表失败:', msg);
    } finally {
      setLoading(false);
    }
  }, [selectedModelId]);

  useEffect(() => {
    refresh();
  }, []);

  const selectModel = useCallback((modelId: string) => {
    setSelectedModelId(modelId);
  }, []);

  const createModel = useCallback(
    async (params: CreateModelParams): Promise<CustomModelItem> => {
      const result = await modelApi.createCustomModel(params);
      await refresh();
      return result;
    },
    [refresh],
  );

  const updateModel = useCallback(
    async (params: UpdateModelParams): Promise<CustomModelItem> => {
      const result = await modelApi.updateCustomModel(params);
      await refresh();
      return result;
    },
    [refresh],
  );

  const deleteModel = useCallback(
    async (modelId: string): Promise<void> => {
      await modelApi.deleteCustomModel(modelId);
      await refresh();
    },
    [refresh],
  );

  const testConnection = useCallback(
    async (params: TestConnectionParams): Promise<TestResult> => {
      const result = await modelApi.testConnection(params);
      return { success: result.success, message: result.message };
    },
    [],
  );

  return {
    models,
    customModels,
    selectedModelId,
    selectModel,
    loading,
    error,
    refresh,
    createModel,
    updateModel,
    deleteModel,
    testConnection,
  };
}
