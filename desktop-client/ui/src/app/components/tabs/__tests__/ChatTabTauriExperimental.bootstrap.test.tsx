/**
 * ChatTabTauriExperimental - Bootstrap Gate 测试（Phase 1.3.e 红线 #7）
 *
 * 覆盖 selectedThreadId === null 时的 thread 自动创建链路：
 * - ready=false：不创建
 * - ready=true + onThreadCreated 缺省：不创建，渲染占位
 * - ready=true + onThreadCreated 提供：调 threadApi.createThread，回调真实 id
 * - ready=true + selectedThreadId 非空：不创建（正常挂载 runtime）
 * - 失败路径：createThread reject 后允许后续重试
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, act, waitFor } from '@testing-library/react';
import { ChatTabTauriExperimental } from '../ChatTabTauriExperimental';
import { EngineReadyProvider } from '../../../hooks/useEngineReady';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn().mockResolvedValue(undefined) }));

type Handler = (event: { payload: unknown }) => void;
const handlers: Handler[] = [];
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn((_event: string, h: Handler) => {
    handlers.push(h);
    return Promise.resolve(() => {});
  }),
}));

const createThreadMock = vi.fn();
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
      createThread: (...args: unknown[]) => createThreadMock(...args),
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

describe('ChatTabTauriExperimental - bootstrap gate', () => {
  beforeEach(() => {
    handlers.length = 0;
    createThreadMock.mockReset();
    vi.clearAllMocks();
  });

  it('未就绪 + threadId=null：不调 createThread，仅渲染启动占位', () => {
    createThreadMock.mockResolvedValue({ id: 'new-1', title: 'x', created_at: '', updated_at: '' });
    const onThreadCreated = vi.fn();

    render(
      <EngineReadyProvider>
        <ChatTabTauriExperimental selectedThreadId={null} onThreadCreated={onThreadCreated} />
      </EngineReadyProvider>,
    );

    expect(screen.getByTestId('chat-runtime-boot-placeholder')).toBeInTheDocument();
    expect(createThreadMock).not.toHaveBeenCalled();
    expect(onThreadCreated).not.toHaveBeenCalled();
  });

  it('就绪 + threadId=null + onThreadCreated 缺省：不创建，渲染引导占位', async () => {
    createThreadMock.mockResolvedValue({ id: 'should-not-happen', title: '', created_at: '', updated_at: '' });

    render(
      <EngineReadyProvider>
        <ChatTabTauriExperimental selectedThreadId={null} />
      </EngineReadyProvider>,
    );
    fireEngineReady();

    await waitFor(() => {
      expect(screen.getByTestId('chat-runtime-bootstrap-placeholder')).toBeInTheDocument();
    });
    expect(createThreadMock).not.toHaveBeenCalled();
  });

  it('就绪 + threadId=null + 提供 onThreadCreated：调 createThread 并回传真实 id', async () => {
    createThreadMock.mockResolvedValue({
      id: 'real-thread-42',
      title: '新对话',
      created_at: '',
      updated_at: '',
    });
    const onThreadCreated = vi.fn();

    render(
      <EngineReadyProvider>
        <ChatTabTauriExperimental selectedThreadId={null} onThreadCreated={onThreadCreated} />
      </EngineReadyProvider>,
    );
    fireEngineReady();

    await waitFor(() => {
      expect(createThreadMock).toHaveBeenCalledTimes(1);
    });
    await waitFor(() => {
      expect(onThreadCreated).toHaveBeenCalledWith('real-thread-42');
    });
  });

  it('就绪 + threadId 非空：完全不走 bootstrap 分支', async () => {
    const onThreadCreated = vi.fn();
    createThreadMock.mockResolvedValue({ id: 'x', title: '', created_at: '', updated_at: '' });

    render(
      <EngineReadyProvider>
        <ChatTabTauriExperimental selectedThreadId="t-existing" onThreadCreated={onThreadCreated} />
      </EngineReadyProvider>,
    );
    fireEngineReady();

    await waitFor(() => {
      expect(screen.getByTestId('thread-root')).toBeInTheDocument();
    });
    expect(createThreadMock).not.toHaveBeenCalled();
    expect(onThreadCreated).not.toHaveBeenCalled();
  });

  it('createThread 失败后允许 threadId 再次变 null 时重试', async () => {
    createThreadMock
      .mockRejectedValueOnce(new Error('backend unavailable'))
      .mockResolvedValueOnce({ id: 'retry-ok', title: '', created_at: '', updated_at: '' });
    const onThreadCreated = vi.fn();

    const { rerender } = render(
      <EngineReadyProvider>
        <ChatTabTauriExperimental selectedThreadId={null} onThreadCreated={onThreadCreated} />
      </EngineReadyProvider>,
    );
    fireEngineReady();

    await waitFor(() => {
      expect(createThreadMock).toHaveBeenCalledTimes(1);
    });
    // 失败后 onThreadCreated 不应触发
    expect(onThreadCreated).not.toHaveBeenCalled();

    // 模拟上层把 threadId 切回 null（例如用户点了"新建对话"按钮）后重试
    rerender(
      <EngineReadyProvider>
        <ChatTabTauriExperimental selectedThreadId="temp" onThreadCreated={onThreadCreated} />
      </EngineReadyProvider>,
    );
    rerender(
      <EngineReadyProvider>
        <ChatTabTauriExperimental selectedThreadId={null} onThreadCreated={onThreadCreated} />
      </EngineReadyProvider>,
    );

    await waitFor(() => {
      expect(createThreadMock).toHaveBeenCalledTimes(2);
    });
    await waitFor(() => {
      expect(onThreadCreated).toHaveBeenCalledWith('retry-ok');
    });
  });
});
