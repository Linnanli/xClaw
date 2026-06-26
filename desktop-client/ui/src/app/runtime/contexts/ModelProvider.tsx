/**
 * ModelProvider — Phase 1.2 AI-SDK 新 Runtime 的模型状态 Provider
 *
 * 职责：
 * - 挂载时加载可用模型列表（`modelApi.getAvailableModels`）
 * - 管理 `selectedModelId`，切换时调用 `modelApi.activateModel`（与旧 `TauriRuntimeProvider` 行为对齐）
 * - 通过既有 `ModelContext`（`app/contexts/ModelContext.tsx`）向下暴露状态，
 *   保证 `<ModelSelector>` 等既有消费者无需任何改动
 *
 * 与旧 Provider 的边界：
 * - 旧 `TauriRuntimeProvider` 内部自管模型状态；迁移到新 Runtime 后该状态由本 Provider 独立承担
 * - 本 Provider 不涉及任何消息 / 流式 / DLP 状态，单一职责
 *
 * 移除计划：Phase 1.5 切换完成后保留，实际替代旧 Provider 的同名逻辑
 */

import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { ModelContext } from '../../contexts/ModelContext';
import { useEngineReady } from '../../hooks/useEngineReady';
import { modelApi, type ModelConfigItem } from '@utils/tauri';
import { tracing } from '../../utils/tracing';

export interface ModelProviderProps {
  /** 初始模型 id；若无效或未提供，挂载后回退到 `is_default` 或第一个模型 */
  initialModelId?: string;
  /** 模型切换回调（UI 层可据此持久化） */
  onModelChange?: (modelId: string) => void;
  /** “自定义模型”按钮点击时触发（由上层打开弹窗） */
  onOpenCustomModelModal?: () => void;
  children: ReactNode;
}

export function ModelProvider({
  initialModelId,
  onModelChange,
  onOpenCustomModelModal,
  children,
}: ModelProviderProps) {
  const [models, setModels] = useState<ModelConfigItem[]>([]);
  const [selectedModelId, setSelectedModelId] = useState<string>(initialModelId ?? '');
  const [loading, setLoading] = useState(true);
  // 用 ref 让闭包始终读到最新的选中 id，避免切换时闭包被冻结
  const selectedModelIdRef = useRef<string>(initialModelId ?? '');
  const { ready, readyKey } = useEngineReady();

  const loadModels = useCallback(async () => {
    try {
      const all = await modelApi.getAvailableModels();
      setModels(all);
      const prevId = selectedModelIdRef.current;
      const resolvedId =
        prevId && all.some((m) => m.model_id === prevId)
          ? prevId
          : (all.find((m) => m.is_default) ?? all[0])?.model_id ?? prevId;
      if (resolvedId !== prevId) {
        selectedModelIdRef.current = resolvedId;
        setSelectedModelId(resolvedId);
        if (resolvedId) {
          onModelChange?.(resolvedId);
        }
      }
    } catch (err) {
      tracing.error('ModelProvider: failed to load models', { error: err });
    } finally {
      setLoading(false);
    }
  }, [onModelChange]);

  useEffect(() => {
    // 引擎就绪前不拉模型：get_available_models 需要 Admin client token。
    // 等 ready 翻转后由 readyKey 触发重拉，避免无客户端身份请求 Admin。
    if (!ready) return;
    void loadModels();
  }, [loadModels, ready, readyKey]);

  const selectModel = useCallback(
    (modelId: string) => {
      // 同步更新 ref + state，保证外层在同一事件循环中发消息能读到新值
      selectedModelIdRef.current = modelId;
      setSelectedModelId(modelId);
      const model = models.find((m) => m.model_id === modelId) ?? null;
      onModelChange?.(modelId);
      modelApi
        .activateModel({
          model_id: modelId,
          api_base_url: model?.api_base_url,
          api_key: model?.api_key,
        })
        .catch((err) => {
          tracing.warn('ModelProvider: activateModel failed', { modelId, error: err });
        });
    },
    [models, onModelChange],
  );

  const openCustomModelModal = useCallback(() => {
    onOpenCustomModelModal?.();
  }, [onOpenCustomModelModal]);

  const value = useMemo(
    () => ({ selectedModelId, models, selectModel, openCustomModelModal, loading }),
    [selectedModelId, models, selectModel, openCustomModelModal, loading],
  );

  return <ModelContext.Provider value={value}>{children}</ModelContext.Provider>;
}
