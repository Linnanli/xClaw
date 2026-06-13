// @vitest-environment happy-dom
import { type ReactElement } from 'react';
import { act } from 'react-dom/test-utils';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { AppConfig, ClientEvent, ClientModelProviderConfig } from '../src/renderer/types';

vi.mock('react-i18next', () => ({
  initReactI18next: {
    type: '3rdParty',
    init: () => undefined,
  },
  useTranslation: () => ({
    t: (key: string, fallback?: string) => fallback ?? key,
  }),
}));

const baseConfig: AppConfig = {
  provider: 'openai',
  apiKey: 'sk-test',
  model: 'gpt-4.1',
  activeProfileKey: 'openai',
  profiles: {},
  activeConfigSetId: 'default',
  configSets: [],
  isConfigured: true,
  theme: 'light',
};

const serverConfig = (selectedModelId: string): ClientModelProviderConfig => ({
  selectedModelId,
  models: [
    {
      modelId: 'gpt-4.1',
      displayName: 'GPT 4.1',
      provider: 'openai',
      isDefault: true,
      capabilities: [],
      apiBaseUrl: 'https://api.openai.com/v1',
      apiFormat: 'openai',
      source: 'test',
      apiKeyConfigured: true,
    },
    {
      modelId: 'gpt-5',
      displayName: 'GPT 5',
      provider: 'openai',
      isDefault: false,
      capabilities: [],
      apiBaseUrl: 'https://api.openai.com/v1',
      apiFormat: 'openai',
      source: 'test',
      apiKeyConfigured: true,
    },
  ],
});

const containers: Array<{ container: HTMLDivElement; root: Root }> = [];

function render(element: ReactElement): HTMLDivElement {
  const container = document.createElement('div');
  document.body.appendChild(container);
  const root = createRoot(container);

  act(() => {
    root.render(element);
  });

  containers.push({ container, root });
  return container;
}

async function waitFor(assertion: () => void): Promise<void> {
  let lastError: unknown;
  for (let attempt = 0; attempt < 40; attempt += 1) {
    try {
      assertion();
      return;
    } catch (error) {
      lastError = error;
      await act(async () => {
        await new Promise((resolve) => setTimeout(resolve, 0));
      });
    }
  }
  throw lastError;
}

function installElectronApiMock(options: {
  invoke: (event: ClientEvent) => Promise<unknown>;
  config?: AppConfig;
}) {
  const electronAPI = {
    on: vi.fn(() => vi.fn()),
    send: vi.fn(),
    invoke: vi.fn(options.invoke),
    getSystemTheme: vi.fn().mockResolvedValue({ shouldUseDarkColors: false }),
    config: {
      get: vi.fn().mockResolvedValue(options.config ?? baseConfig),
      isConfigured: vi.fn().mockResolvedValue(true),
      listModels: vi.fn().mockResolvedValue([
        { id: 'gpt-4.1', name: 'GPT 4.1' },
        { id: 'gpt-5', name: 'GPT 5' },
      ]),
      save: vi.fn().mockResolvedValue({
        success: true,
        config: { ...(options.config ?? baseConfig), model: 'gpt-5' },
      }),
    },
  };

  Object.defineProperty(window, 'electronAPI', {
    configurable: true,
    value: electronAPI,
  });

  return electronAPI;
}

beforeEach(() => {
  vi.resetModules();
  (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
});

afterEach(() => {
  for (const { container, root } of containers.splice(0)) {
    act(() => {
      root.unmount();
    });
    container.remove();
  }
  Reflect.deleteProperty(window, 'electronAPI');
  vi.restoreAllMocks();
});

describe('useClientGuiModelSelector behavior', () => {
  it('selects app-server models and commits the returned modelProvider config', async () => {
    const electronAPI = installElectronApiMock({
      invoke: async (event) => {
        if (event.type === 'modelProvider.list') {
          return serverConfig('gpt-4.1');
        }
        if (event.type === 'modelProvider.selectForNextTurn') {
          return serverConfig(event.payload.modelId);
        }
        throw new Error(`Unexpected event ${event.type}`);
      },
    });
    const { AssistantModelSelector } =
      await import('../src/renderer/components/assistant-ui/AssistantModelSelector');
    const { useAppStore } = await import('../src/renderer/store');

    const container = render(<AssistantModelSelector />);

    await waitFor(() => expect(container.textContent).toContain('GPT 4.1'));

    await act(async () => {
      container
        .querySelector<HTMLButtonElement>('[data-testid="model-selector-trigger"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    await act(async () => {
      container
        .querySelector<HTMLButtonElement>('[data-testid="model-option-gpt-5"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(electronAPI.invoke).toHaveBeenCalledWith({
      type: 'modelProvider.selectForNextTurn',
      payload: { modelId: 'gpt-5' },
    });
    await waitFor(() =>
      expect(useAppStore.getState().modelProviderConfig?.selectedModelId).toBe('gpt-5')
    );
  });

  it('falls back to config-backed model discovery and persists config selections', async () => {
    const electronAPI = installElectronApiMock({
      invoke: async (event) => {
        if (event.type === 'modelProvider.list') {
          throw new Error('server unavailable');
        }
        throw new Error(`Unexpected event ${event.type}`);
      },
    });
    const { AssistantModelSelector } =
      await import('../src/renderer/components/assistant-ui/AssistantModelSelector');

    const container = render(<AssistantModelSelector />);

    await waitFor(() => expect(container.textContent).toContain('GPT 4.1'));

    await act(async () => {
      container
        .querySelector<HTMLButtonElement>('[data-testid="model-selector-trigger"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    await act(async () => {
      container
        .querySelector<HTMLButtonElement>('[data-testid="model-option-gpt-5"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(electronAPI.config.save).toHaveBeenCalledWith({ model: 'gpt-5' });
  });

  it('shows a global notice when model selection fails', async () => {
    installElectronApiMock({
      invoke: async (event) => {
        if (event.type === 'modelProvider.list') {
          return serverConfig('gpt-4.1');
        }
        if (event.type === 'modelProvider.selectForNextTurn') {
          throw new Error('selection failed');
        }
        throw new Error(`Unexpected event ${event.type}`);
      },
    });
    const { AssistantModelSelector } =
      await import('../src/renderer/components/assistant-ui/AssistantModelSelector');
    const { useAppStore } = await import('../src/renderer/store');

    const container = render(<AssistantModelSelector />);

    await waitFor(() => expect(container.textContent).toContain('GPT 4.1'));

    await act(async () => {
      container
        .querySelector<HTMLButtonElement>('[data-testid="model-selector-trigger"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    await act(async () => {
      container
        .querySelector<HTMLButtonElement>('[data-testid="model-option-gpt-5"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    await waitFor(() =>
      expect(useAppStore.getState().globalNotice?.message).toBe('selection failed')
    );
  });
});
