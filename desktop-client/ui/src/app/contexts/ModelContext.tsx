/**
 * ModelContext — 对话界面模型选择的共享状态
 *
 * 职责：
 * - 持有当前选中的 modelId 和可用模型列表
 * - 由 TauriRuntimeProvider 提供，供 Composer 内的 ModelSelector 消费
 * - 与 DlpContext 解耦，单一职责
 *
 * 设计决策：
 * - 不在 Composer 内部调用 useModelConfig（避免 hook 在 assistant-ui 内部调用的限制）
 * - TauriRuntimeProvider 通过 modelIdRef 动态响应切换，无需 key 重置 runtime
 */

import { createContext, useContext } from 'react';
import type { ModelConfigItem } from '@utils/tauri';

export interface ModelContextValue {
  /** 当前选中的模型 ID */
  selectedModelId: string;
  /** 所有可用模型 */
  models: ModelConfigItem[];
  /** 切换模型 */
  selectModel: (modelId: string) => void;
  /** 打开自定义模型弹窗 */
  openCustomModelModal: () => void;
  /** 是否正在加载模型列表 */
  loading: boolean;
}

export const ModelContext = createContext<ModelContextValue>({
  selectedModelId: '',
  models: [],
  selectModel: () => {},
  openCustomModelModal: () => {},
  loading: false,
});

export const useModelContext = () => useContext(ModelContext);
