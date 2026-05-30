/**
 * ChatTabTauriExperimental - Engine Ready Gate 测试
 *
 * 覆盖：
 * - 引擎未就绪时渲染占位（placeholder），不挂载 ChatRuntimeProvider
 * - 引擎就绪后挂载 ChatRuntimeProvider（通过 Thread 可见性近似验证）
 *
 * 关注点：Phase 1.2 Step C 的唯一业务约束 —— transport 在 IPC 通道未准备好时
 * 不应该被构造并触发 listen/invoke。
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, act, waitFor } from '@testing-library/react';
import { ChatTabTauriExperimental } from '../ChatTabTauriExperimental';
import { EngineReadyProvider } from '../../../hooks/useEngineReady';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn().mockResolvedValue(undefined) }));

// listen 被 useEngineReady / ApprovalProvider / TauriChatTransport 订阅；
// 我们只关心 EngineReadyProvider 订阅到 connection_status 后翻转 ready
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

// Thread 内有复杂的 assistant-ui 依赖，测试里 stub 掉，只需判断 presence
vi.mock('@components/assistant-ui/thread', () => ({
  Thread: () => <div data-testid="thread-root">thread</div>,
}));

describe('ChatTabTauriExperimental - engine ready 门控', () => {
  beforeEach(() => {
    handlers.length = 0;
    vi.clearAllMocks();
  });

  it('引擎未就绪时渲染占位且不渲染 Thread', () => {
    render(
      <EngineReadyProvider>
        <ChatTabTauriExperimental selectedThreadId="t-1" />
      </EngineReadyProvider>,
    );
    expect(screen.getByTestId('chat-runtime-boot-placeholder')).toBeInTheDocument();
    expect(screen.queryByTestId('thread-root')).toBeNull();
  });

  it('收到 connection_status connected=true 后挂载 Thread', async () => {
    render(
      <EngineReadyProvider>
        <ChatTabTauriExperimental selectedThreadId="t-1" />
      </EngineReadyProvider>,
    );
    expect(screen.queryByTestId('thread-root')).toBeNull();

    // 模拟引擎就绪事件
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

    expect(screen.queryByTestId('chat-runtime-boot-placeholder')).toBeNull();
    // ThreadHistoryLoader 异步加载历史后才挂载 Thread
    await waitFor(() => {
      expect(screen.getByTestId('thread-root')).toBeInTheDocument();
    });
  });
});
