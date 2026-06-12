import type { ClientModelProviderConfig, ProviderModelInfo } from '../../types';

export interface AssistantModelOption {
  modelId: string;
  displayName: string;
  isDefault: boolean;
}

export type AssistantModelSelectorMode = 'loading' | 'server' | 'config' | 'unavailable';

export const normalizeServerModelOptions = (
  config: ClientModelProviderConfig
): AssistantModelOption[] =>
  config.models.map((model, index) => ({
    modelId: model.modelId,
    displayName: model.displayName || model.modelId,
    isDefault: model.isDefault || index === 0,
  }));

export const normalizeConfigModelOptions = (models: ProviderModelInfo[]): AssistantModelOption[] =>
  models.map((model, index) => ({
    modelId: model.id,
    displayName: model.name || model.id,
    isDefault: index === 0,
  }));

export function resolveModelDisplayName(
  selectedModelId: string,
  options: AssistantModelOption[],
  fallbackLabel: string
): string {
  const selected = options.find((model) => model.modelId === selectedModelId);
  if (selected?.displayName) {
    return selected.displayName;
  }
  return selectedModelId || fallbackLabel;
}
