import { useCallback, useEffect, useMemo, useState } from 'react';
import { useAppStore } from '../store';
import { useAppConfig } from '../store/selectors';
import { useIPC } from './useIPC';
import {
  normalizeConfigModelOptions,
  normalizeServerModelOptions,
  type AssistantModelOption,
  type AssistantModelSelectorMode,
} from '../components/assistant-ui/model-selector-utils';
import type { ClientModelProviderConfig } from '../types';

interface UseClientGuiModelSelectorResult {
  mode: AssistantModelSelectorMode;
  models: AssistantModelOption[];
  selectedModelId: string;
  loading: boolean;
  error: string | null;
  selectModel: (modelId: string) => Promise<void>;
  openSettings: () => void;
}

/**
 * Model selector state adapter for client-gui.
 *
 * Committed state sources:
 * - app-server mode: modelProviderConfig from renderer store, updated by modelProvider.changed
 * - config-backed mode: appConfig.model from renderer store, updated by config.status / config.save
 */
export function useClientGuiModelSelector(): UseClientGuiModelSelectorResult {
  const { invoke, isElectron } = useIPC();
  const appConfig = useAppConfig();
  const modelProviderConfig = useAppStore((state) => state.modelProviderConfig);
  const setAppConfig = useAppStore((state) => state.setAppConfig);
  const setModelProviderConfig = useAppStore((state) => state.setModelProviderConfig);
  const setGlobalNotice = useAppStore((state) => state.setGlobalNotice);
  const setShowSettings = useAppStore((state) => state.setShowSettings);
  const setSettingsTab = useAppStore((state) => state.setSettingsTab);

  const [mode, setMode] = useState<AssistantModelSelectorMode>('loading');
  const [models, setModels] = useState<AssistantModelOption[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchModels = useCallback(async () => {
    if (!isElectron || !window.electronAPI) {
      setMode('unavailable');
      setModels([]);
      setLoading(false);
      return;
    }

    setLoading(true);
    setError(null);

    try {
      const serverConfig = await invoke<ClientModelProviderConfig>({
        type: 'modelProvider.list',
        payload: {},
      });

      setMode('server');
      setModels(normalizeServerModelOptions(serverConfig));
      setModelProviderConfig(serverConfig);
      return;
    } catch (serverError) {
      console.log('[useClientGuiModelSelector] modelProvider.list unavailable:', serverError);
    }

    try {
      const provider = appConfig?.provider;
      const apiKey = appConfig?.apiKey?.trim();
      const baseUrl = appConfig?.baseUrl?.trim() || undefined;

      if (!provider || !apiKey) {
        setMode('unavailable');
        setModels([]);
        return;
      }

      const discovered = await window.electronAPI.config.listModels({
        provider,
        apiKey,
        baseUrl,
      });

      setMode('config');
      setModels(normalizeConfigModelOptions(discovered));
      return;
    } catch (configError) {
      const message = configError instanceof Error ? configError.message : 'No models available';
      setMode('unavailable');
      setModels([]);
      setError(message);
      setGlobalNotice({
        id: `notice-model-selector-${Date.now()}`,
        type: 'error',
        message,
      });
    } finally {
      setLoading(false);
    }
  }, [appConfig?.apiKey, appConfig?.baseUrl, appConfig?.provider, invoke, isElectron, setGlobalNotice, setModelProviderConfig]);

  useEffect(() => {
    void fetchModels();
  }, [fetchModels]);

  const selectedModelId = useMemo(() => {
    if (mode === 'server') {
      return modelProviderConfig?.selectedModelId || appConfig?.model || '';
    }
    return appConfig?.model || modelProviderConfig?.selectedModelId || '';
  }, [appConfig?.model, mode, modelProviderConfig?.selectedModelId]);

  const openSettings = useCallback(() => {
    setSettingsTab('api');
    setShowSettings(true);
  }, [setSettingsTab, setShowSettings]);

  const selectModel = useCallback(
    async (modelId: string) => {
      if (!modelId) return;

      try {
        if (mode === 'server') {
          const config = await invoke<ClientModelProviderConfig>({
            type: 'modelProvider.selectForNextTurn',
            payload: { modelId },
          });
          setModelProviderConfig(config);
        } else if (mode === 'config') {
          if (!window.electronAPI) {
            throw new Error('Electron API unavailable');
          }

          const result = await window.electronAPI.config.save({ model: modelId });
          if (!result?.success) {
            throw new Error('Failed to save model selection');
          }
          setAppConfig(result.config);
        } else {
          throw new Error('No models available');
        }
      } catch (selectionError) {
        const message =
          selectionError instanceof Error ? selectionError.message : 'No models available';
        setError(message);
        setGlobalNotice({
          id: `notice-model-select-${Date.now()}`,
          type: 'error',
          message,
        });
        return;
      }
    },
    [invoke, mode, setAppConfig, setGlobalNotice, setModelProviderConfig]
  );

  return {
    mode,
    models,
    selectedModelId,
    loading,
    error,
    selectModel,
    openSettings,
  };
}
