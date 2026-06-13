// @vitest-environment happy-dom
import { type ReactElement } from 'react';
import { act } from 'react-dom/test-utils';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { AppConfig, ClientModelProviderConfig, ServerEvent } from '../src/renderer/types';

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

const changedConfig: ClientModelProviderConfig = {
  selectedModelId: 'gpt-5',
  models: [
    {
      modelId: 'gpt-5',
      displayName: 'GPT 5',
      provider: 'openai',
      isDefault: true,
      capabilities: [],
      apiBaseUrl: 'https://api.openai.com/v1',
      apiFormat: 'openai',
      source: 'test',
      apiKeyConfigured: true,
    },
  ],
};

const containers: Array<{ container: HTMLDivElement; root: Root }> = [];

function render(element: ReactElement): void {
  const container = document.createElement('div');
  document.body.appendChild(container);
  const root = createRoot(container);

  act(() => {
    root.render(element);
  });

  containers.push({ container, root });
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

describe('useIPC model provider events', () => {
  it('stores modelProvider.changed payload in renderer state', async () => {
    let serverEventListener: ((event: ServerEvent) => void) | null = null;
    Object.defineProperty(window, 'electronAPI', {
      configurable: true,
      value: {
        on: vi.fn((listener: (event: ServerEvent) => void) => {
          serverEventListener = listener;
          return vi.fn();
        }),
        send: vi.fn(),
        invoke: vi.fn(),
        getSystemTheme: vi.fn().mockResolvedValue({ shouldUseDarkColors: false }),
        config: {
          get: vi.fn().mockResolvedValue(baseConfig),
          isConfigured: vi.fn().mockResolvedValue(true),
        },
      },
    });

    const { useIPC } = await import('../src/renderer/hooks/useIPC');
    const { useAppStore } = await import('../src/renderer/store');

    function Probe(): null {
      useIPC();
      return null;
    }

    render(<Probe />);

    expect(serverEventListener).not.toBeNull();

    await act(async () => {
      serverEventListener?.({ type: 'modelProvider.changed', payload: changedConfig });
      await Promise.resolve();
    });

    expect(useAppStore.getState().modelProviderConfig).toEqual(changedConfig);
  });
});
