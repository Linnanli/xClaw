import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render, screen, act } from '@testing-library/react';
import { ConnectionStatus } from '../ConnectionStatus';

// Mock Tauri event API
const mockListenHandlers: Record<string, ((event: any) => void)[]> = {};
const mockUnlisten = vi.fn();

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn((eventName: string, handler: (event: any) => void) => {
    if (!mockListenHandlers[eventName]) {
      mockListenHandlers[eventName] = [];
    }
    mockListenHandlers[eventName].push(handler);
    return Promise.resolve(mockUnlisten);
  }),
}));

vi.mock('../../../contexts/ThemeContext', () => ({
  useTheme: () => ({ theme: 'light' }),
}));

// 触发 chat-event 的辅助函数
function emitChatEvent(payload: unknown) {
  const handlers = mockListenHandlers['chat-event'] ?? [];
  handlers.forEach((h) => h({ payload }));
}

describe('ConnectionStatus', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
    // 清空 handler 注册表
    Object.keys(mockListenHandlers).forEach((k) => delete mockListenHandlers[k]);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('应该显示引擎启动中状态（初始状态）', () => {
    render(<ConnectionStatus />);
    expect(screen.getByText('引擎启动中')).toBeInTheDocument();
    expect(screen.getByTestId('connection-dot')).toHaveClass('bg-yellow-400');
  });

  it('收到非错误事件后应显示引擎运行中状态', async () => {
    render(<ConnectionStatus />);

    await act(async () => {
      emitChatEvent({ type: 'response', content: 'hello' });
    });

    expect(screen.getByText('引擎运行中')).toBeInTheDocument();
    expect(screen.getByTestId('connection-dot')).toHaveClass('bg-green-400');
  });

  it('收到引擎错误事件后应显示引擎异常状态', async () => {
    render(<ConnectionStatus />);

    await act(async () => {
      emitChatEvent({ Error: { message: '启动失败', code: 'ENGINE_STARTUP_FAILED' } });
    });

    expect(screen.getByText('引擎异常')).toBeInTheDocument();
    expect(screen.getByTestId('connection-dot')).toHaveClass('bg-red-400');
  });

  it('5 秒超时后应自动切换到引擎运行中', async () => {
    render(<ConnectionStatus />);

    await act(async () => {
      vi.advanceTimersByTime(5000);
    });

    expect(screen.getByText('引擎运行中')).toBeInTheDocument();
  });

  it('已切换到运行中后超时不应改变状态', async () => {
    render(<ConnectionStatus />);

    await act(async () => {
      emitChatEvent({ type: 'response' });
    });

    await act(async () => {
      vi.advanceTimersByTime(5000);
    });

    expect(screen.getByText('引擎运行中')).toBeInTheDocument();
  });

  it('应注册 chat-event 监听器', async () => {
    const { listen } = await import('@tauri-apps/api/event');
    render(<ConnectionStatus />);
    expect(listen).toHaveBeenCalledWith('chat-event', expect.any(Function));
  });

  it('卸载时应调用 unlisten 清理', async () => {
    const { unmount } = render(<ConnectionStatus />);

    // 等待 listen promise resolve，unlisten 函数存入 ref
    await act(async () => {
      await Promise.resolve();
    });

    unmount();

    expect(mockUnlisten).toHaveBeenCalled();
  });
});
