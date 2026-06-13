// @vitest-environment happy-dom
import { type ReactElement } from 'react';
import { act } from 'react-dom/test-utils';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { ChatView } from '../src/renderer/components/ChatView';
import { useAppStore } from '../src/renderer/store';

vi.mock('react-i18next', () => ({
  initReactI18next: {
    type: '3rdParty',
    init: () => undefined,
  },
  useTranslation: () => ({
    t: (key: string, values?: Record<string, unknown> | string) =>
      typeof values === 'string' ? values : key,
  }),
}));

class ResizeObserverMock {
  observe(): void {}
  unobserve(): void {}
  disconnect(): void {}
}

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

beforeAll(() => {
  (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
  globalThis.ResizeObserver = ResizeObserverMock as unknown as typeof ResizeObserver;
});

beforeEach(() => {
  useAppStore.setState(useAppStore.getInitialState());
  useAppStore.setState({
    activeSessionId: 'session-1',
    sessions: [
      {
        id: 'session-1',
        title: 'Assistant UI session',
        status: 'idle',
        createdAt: 1,
        updatedAt: 1,
        mountedPaths: [],
        allowedTools: [],
        memoryEnabled: true,
      },
    ],
    sessionStates: {
      'session-1': {
        messages: [
          {
            id: 'message-1',
            sessionId: 'session-1',
            role: 'assistant',
            content: [{ type: 'text', text: 'hello from assistant-ui' }],
            timestamp: 1,
          },
        ],
        partialMessage: '',
        partialThinking: '',
        pendingTurns: [],
        activeTurn: null,
        executionClock: { startAt: null, endAt: null },
        traceSteps: [],
        contextWindow: 0,
      },
    },
  });
});

afterEach(() => {
  for (const { container, root } of containers.splice(0)) {
    act(() => {
      root.unmount();
    });
    container.remove();
  }
  vi.restoreAllMocks();
});

describe('ChatView assistant-ui integration', () => {
  it('renders the chat page through the assistant shell, thread, model selector slot, and composer', async () => {
    const container = render(<ChatView />);

    await act(async () => {
      await Promise.resolve();
    });

    expect(container.querySelector('[data-assistant-chat-shell="client-gui"]')).not.toBeNull();
    expect(container.querySelector('[data-message-count="1"]')).not.toBeNull();
    expect(container.querySelector('[data-assistant-composer="client-gui"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="model-selector-trigger"]')).not.toBeNull();
    expect(container.textContent).toContain('Assistant UI session');
    expect(container.textContent).toContain('hello from assistant-ui');
  });

  it('keeps the Claude-style shell layout on the rendered assistant-ui surface', async () => {
    const container = render(<ChatView />);

    await act(async () => {
      await Promise.resolve();
    });

    const header = container.querySelector('[data-assistant-chat-header="client-gui"]');
    const messages = container.querySelector('[data-assistant-message-container="client-gui"]');
    const composerBar = container.querySelector('[data-assistant-composer-bar="client-gui"]');
    const composerContainer = container.querySelector(
      '[data-assistant-composer-container="client-gui"]'
    );
    const composer = container.querySelector('[data-assistant-composer="client-gui"]');

    expect(header?.textContent).toContain('Open Cowork');
    expect(header?.className).toContain('bg-background/88');
    expect(header?.className).toContain('border-border-muted');
    expect(messages?.className).toContain('max-w-[920px]');
    expect(messages?.className).toContain('lg:px-8');
    expect(composerContainer?.className).toContain('max-w-[920px]');
    expect(composerContainer?.className).toContain('lg:px-8');
    expect(composerBar?.className).toContain('bg-background/92');
    expect(composer?.className).toContain('rounded-[1.75rem]');
    expect(composer?.className).toContain('shadow-soft');
  });
});
