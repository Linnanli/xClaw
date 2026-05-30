/**
 * ChatTabTauriExperimental - OutboundCommand Bridge 测试（Phase 1.4）
 *
 * 验证外部指令（"常见问题"等场景）能够通过 useComposerRuntime 注入到
 * composer 输入框并触发发送，复用 Composer 的 DLP 拦截链路。
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, act, waitFor } from '@testing-library/react';
import { ChatTabTauriExperimental } from '../ChatTabTauriExperimental';
import { EngineReadyProvider } from '../../../hooks/useEngineReady';
import type { ChatCommand } from '../../../types/chatCommand';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn().mockResolvedValue(undefined) }));

type Handler = (event: { payload: unknown }) => void;
const handlers: Handler[] = [];
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn((_event: string, h: Handler) => {
    handlers.push(h);
    return Promise.resolve(() => {});
  }),
}));

vi.mock('@utils/tauri', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@utils/tauri')>();
  return {
    ...actual,
    modelApi: {
      getAvailableModels: vi.fn().mockResolvedValue([]),
      activateModel: vi.fn().mockResolvedValue(undefined),
    },
    threadApi: {
      ...actual.threadApi,
      getMessages: vi.fn().mockResolvedValue([]),
    },
  };
});

vi.mock('@utils/tokenManager', () => ({
  TokenManager: { getToken: vi.fn().mockResolvedValue('token') },
}));

vi.mock('@components/assistant-ui/thread', () => ({
  Thread: () => <div data-testid="thread-root">thread</div>,
}));

// Mock @assistant-ui/react 的 useComposerRuntime —— 实际 runtime 在测试环境
// 难以挂起；这里 stub 出可观测的 setText/send 供断言。
const composerSetText = vi.fn();
const composerSend = vi.fn();
vi.mock('@assistant-ui/react', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@assistant-ui/react')>();
  return {
    ...actual,
    useComposerRuntime: () => ({ setText: composerSetText, send: composerSend }),
  };
});

function fireEngineReady() {
  act(() => {
    for (const h of handlers) {
      h({
        payload: {
          type: 'data-custom',
          data: { type: 'connection_status', connected: true },
        },
      });
    }
  });
}

describe('ChatTabTauriExperimental - OutboundCommand Bridge', () => {
  beforeEach(() => {
    handlers.length = 0;
    composerSetText.mockReset();
    composerSend.mockReset();
    vi.clearAllMocks();
  });

  it('收到 send_text 指令应注入 composer 并触发 send，回调 onConsume', async () => {
    const onConsume = vi.fn();
    const cmd: ChatCommand = { id: 'cmd-1', kind: 'send_text', text: '帮我写一个 Hello World' };

    const { rerender } = render(
      <EngineReadyProvider>
        <ChatTabTauriExperimental
          selectedThreadId="t-1"
          outboundCommand={null}
          onOutboundCommandHandled={onConsume}
        />
      </EngineReadyProvider>,
    );
    fireEngineReady();
    // 等 ThreadHistoryLoader 异步挂起
    await waitFor(() => expect(composerSetText).not.toHaveBeenCalled());

    // 注入指令
    rerender(
      <EngineReadyProvider>
        <ChatTabTauriExperimental
          selectedThreadId="t-1"
          outboundCommand={cmd}
          onOutboundCommandHandled={onConsume}
        />
      </EngineReadyProvider>,
    );

    await waitFor(() => {
      expect(composerSetText).toHaveBeenCalledWith('帮我写一个 Hello World');
      expect(composerSend).toHaveBeenCalledTimes(1);
      expect(onConsume).toHaveBeenCalledWith('cmd-1');
    });
  });

  it('同一 command id 重复传入不应重复发送（防 strict-mode 双挂载）', async () => {
    const onConsume = vi.fn();
    const cmd: ChatCommand = { id: 'cmd-dup', kind: 'send_text', text: 'hi' };

    const { rerender } = render(
      <EngineReadyProvider>
        <ChatTabTauriExperimental
          selectedThreadId="t-1"
          outboundCommand={cmd}
          onOutboundCommandHandled={onConsume}
        />
      </EngineReadyProvider>,
    );
    fireEngineReady();
    await waitFor(() => expect(composerSend).toHaveBeenCalledTimes(1));

    // 再次传入同一对象（模拟父组件重渲染但 command 未消费完）
    rerender(
      <EngineReadyProvider>
        <ChatTabTauriExperimental
          selectedThreadId="t-1"
          outboundCommand={cmd}
          onOutboundCommandHandled={onConsume}
        />
      </EngineReadyProvider>,
    );
    // 不应再次触发 send
    expect(composerSend).toHaveBeenCalledTimes(1);
    expect(onConsume).toHaveBeenCalledTimes(1);
  });

  it('command=null 时不调 setText/send', async () => {
    const onConsume = vi.fn();

    render(
      <EngineReadyProvider>
        <ChatTabTauriExperimental
          selectedThreadId="t-1"
          outboundCommand={null}
          onOutboundCommandHandled={onConsume}
        />
      </EngineReadyProvider>,
    );
    fireEngineReady();
    await waitFor(() => expect(composerSetText).not.toHaveBeenCalled());
    expect(composerSend).not.toHaveBeenCalled();
    expect(onConsume).not.toHaveBeenCalled();
  });
});
