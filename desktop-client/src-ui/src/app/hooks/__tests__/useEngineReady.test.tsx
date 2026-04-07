/**
 * useEngineReady hook 测试
 *
 * 覆盖维度（AGENTS.md 测试矩阵）：
 * - 单元测试：初始状态、引擎就绪事件响应
 * - 失败路径：非 connection_status 事件不触发、connected=false 不触发
 * - 契约测试：readyKey 单调递增、ready 状态正确
 * - 安全审计：cleanup 时 unlisten 被调用
 */

import { renderHook, act, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest';
import { EngineReadyProvider, useEngineReady } from '../useEngineReady';
import type { ReactNode } from 'react';

// ── Mock ──────────────────────────────────────────────────────────────────────

const mockUnlisten = vi.fn();
let capturedListener: ((event: { payload: unknown }) => void) | null = null;

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn((_channel: string, handler: (event: { payload: unknown }) => void) => {
    capturedListener = handler;
    return Promise.resolve(mockUnlisten);
  }),
}));

// ── 辅助函数 ──────────────────────────────────────────────────────────────────

function wrapper({ children }: { children: ReactNode }) {
  return <EngineReadyProvider>{children}</EngineReadyProvider>;
}

function emitConnectionStatus(connected: boolean) {
  capturedListener?.({ payload: { type: 'connection_status', connected } });
}

function emitOtherEvent(type: string) {
  capturedListener?.({ payload: { type } });
}

// ── 测试 ──────────────────────────────────────────────────────────────────────

describe('useEngineReady', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    capturedListener = null;
  });

  // ── 单元测试：初始状态 ──

  it('初始状态 ready=false, readyKey=0', () => {
    const { result } = renderHook(() => useEngineReady(), { wrapper });
    expect(result.current.ready).toBe(false);
    expect(result.current.readyKey).toBe(0);
  });

  // ── 单元测试：引擎就绪事件 ──

  it('收到 connection_status { connected: true } 后 ready 变为 true', async () => {
    const { result } = renderHook(() => useEngineReady(), { wrapper });

    await waitFor(() => expect(capturedListener).not.toBeNull());

    act(() => emitConnectionStatus(true));

    expect(result.current.ready).toBe(true);
  });

  it('收到 connection_status { connected: true } 后 readyKey 自增', async () => {
    const { result } = renderHook(() => useEngineReady(), { wrapper });

    await waitFor(() => expect(capturedListener).not.toBeNull());

    act(() => emitConnectionStatus(true));
    expect(result.current.readyKey).toBe(1);

    act(() => emitConnectionStatus(true));
    expect(result.current.readyKey).toBe(2);
  });

  // ── 失败路径：不应触发的事件 ──

  it('connected=false 不改变 ready 状态', async () => {
    const { result } = renderHook(() => useEngineReady(), { wrapper });

    await waitFor(() => expect(capturedListener).not.toBeNull());

    act(() => emitConnectionStatus(false));

    expect(result.current.ready).toBe(false);
    expect(result.current.readyKey).toBe(0);
  });

  it('非 connection_status 事件不触发 readyKey 自增', async () => {
    const { result } = renderHook(() => useEngineReady(), { wrapper });

    await waitFor(() => expect(capturedListener).not.toBeNull());

    act(() => emitOtherEvent('response'));
    act(() => emitOtherEvent('thinking'));
    act(() => emitOtherEvent('error'));

    expect(result.current.readyKey).toBe(0);
    expect(result.current.ready).toBe(false);
  });

  // ── 契约测试：readyKey 单调递增 ──

  it('多次就绪事件 readyKey 单调递增', async () => {
    const { result } = renderHook(() => useEngineReady(), { wrapper });

    await waitFor(() => expect(capturedListener).not.toBeNull());

    for (let i = 1; i <= 5; i++) {
      act(() => emitConnectionStatus(true));
      expect(result.current.readyKey).toBe(i);
    }
  });

  // ── 安全审计：cleanup 时 unlisten 被调用 ──

  it('组件卸载时调用 unlisten 清理事件监听', async () => {
    const { unmount } = renderHook(() => useEngineReady(), { wrapper });

    await waitFor(() => expect(capturedListener).not.toBeNull());

    unmount();

    await waitFor(() => expect(mockUnlisten).toHaveBeenCalledOnce());
  });

  // ── 契约测试：Provider 外使用返回默认值 ──

  it('在 EngineReadyProvider 外使用返回默认值', () => {
    const { result } = renderHook(() => useEngineReady());
    expect(result.current.ready).toBe(false);
    expect(result.current.readyKey).toBe(0);
  });
});
