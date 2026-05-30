/**
 * ThreadHistoryLoader 集成测试
 *
 * 覆盖：
 * 1. threadId=null → 直接挂载 ChatRuntimeProvider 且 initialMessages 为空
 * 2. 加载中 → 展示占位符，不挂载 ChatRuntimeProvider
 * 3. 加载成功 → 挂载 ChatRuntimeProvider，initialMessages 为映射结果
 * 4. threadApi 抛错 → 降级挂载空 runtime，不阻塞 UI
 * 5. threadId 切换 → 触发重新加载 + 强制 ChatRuntimeProvider key 重建
 */

import { render, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ReactNode } from 'react';

const { getMessagesMock, chatRuntimeProviderMock, approvalProviderMock } = vi.hoisted(() => ({
  getMessagesMock: vi.fn(),
  chatRuntimeProviderMock: vi.fn((props: { children: ReactNode; [k: string]: unknown }) => (
    <div data-testid="chat-runtime-provider" data-thread-id={String(props.threadId ?? '')}>
      <span data-testid="initial-messages-count">
        {Array.isArray(props.initialMessages) ? props.initialMessages.length : 0}
      </span>
      {props.children}
    </div>
  )),
  approvalProviderMock: vi.fn((props: { children: ReactNode; [k: string]: unknown }) => {
    const approvals = Array.isArray(props.initialPendingApprovals)
      ? props.initialPendingApprovals
      : [];
    return (
      <div
        data-testid="approval-provider"
        data-thread-id={String(props.threadId ?? '')}
        data-approvals-count={String(approvals.length)}
      >
        {props.children}
      </div>
    );
  }),
}));

vi.mock('../../utils/tauri', () => ({
  threadApi: { getMessages: getMessagesMock },
}));
vi.mock('../../utils/tracing', () => ({
  tracing: { error: vi.fn(), info: vi.fn(), warn: vi.fn(), debug: vi.fn() },
}));
vi.mock('../ChatRuntimeProvider', () => ({
  ChatRuntimeProvider: chatRuntimeProviderMock,
}));
vi.mock('../contexts/ApprovalProvider', () => ({
  ApprovalProvider: approvalProviderMock,
}));

import { ThreadHistoryLoader } from '../ThreadHistoryLoader';

describe('ThreadHistoryLoader', () => {
  beforeEach(() => {
    getMessagesMock.mockReset();
    chatRuntimeProviderMock.mockClear();
    approvalProviderMock.mockClear();
  });

  it('threadId=null 时直接挂载空 ChatRuntimeProvider，不调用 getMessages', () => {
    const { getByTestId, queryByTestId } = render(
      <ThreadHistoryLoader threadId={null}>
        <div>content</div>
      </ThreadHistoryLoader>,
    );

    expect(getMessagesMock).not.toHaveBeenCalled();
    expect(queryByTestId('chat-runtime-history-loading')).toBeNull();
    expect(getByTestId('chat-runtime-provider')).toBeTruthy();
    expect(getByTestId('initial-messages-count').textContent).toBe('0');
  });

  it('加载中展示占位符，不挂载 ChatRuntimeProvider', () => {
    let resolver: (value: unknown[]) => void = () => {};
    getMessagesMock.mockImplementation(
      () => new Promise<unknown[]>((resolve) => { resolver = resolve; }),
    );

    const { getByTestId, queryByTestId } = render(
      <ThreadHistoryLoader threadId="t1">
        <div>content</div>
      </ThreadHistoryLoader>,
    );

    expect(getByTestId('chat-runtime-history-loading')).toBeTruthy();
    expect(queryByTestId('chat-runtime-provider')).toBeNull();
    // 释放 promise 避免 unhandled
    resolver([]);
  });

  it('加载成功后挂载 ChatRuntimeProvider，initialMessages 来自 mapper', async () => {
    getMessagesMock.mockResolvedValue([
      { id: 'u1', thread_id: 't1', role: 'user', content: 'hi', created_at: '2025-01-01T00:00:00Z' },
      { id: 'a1', thread_id: 't1', role: 'assistant', content: 'hello', created_at: '2025-01-01T00:00:01Z' },
    ]);

    const { getByTestId } = render(
      <ThreadHistoryLoader threadId="t1" modelId="gpt-4">
        <div>content</div>
      </ThreadHistoryLoader>,
    );

    await waitFor(() => {
      expect(getByTestId('chat-runtime-provider')).toBeTruthy();
    });
    expect(getByTestId('initial-messages-count').textContent).toBe('2');

    const lastCall = chatRuntimeProviderMock.mock.calls.at(-1)?.[0] as Record<string, unknown>;
    expect(lastCall?.threadId).toBe('t1');
    expect(lastCall?.modelId).toBe('gpt-4');
  });

  it('getMessages 抛错时降级挂载空 runtime，tracing.error 被调用', async () => {
    getMessagesMock.mockRejectedValue(new Error('boom'));

    const { getByTestId } = render(
      <ThreadHistoryLoader threadId="t1">
        <div>content</div>
      </ThreadHistoryLoader>,
    );

    await waitFor(() => {
      expect(getByTestId('chat-runtime-provider')).toBeTruthy();
    });
    expect(getByTestId('initial-messages-count').textContent).toBe('0');
  });

  it('threadId 切换时重新加载并传递新 threadId', async () => {
    getMessagesMock.mockImplementation(async (id: string) => [
      {
        id: `u-${id}`,
        thread_id: id,
        role: 'user',
        content: `from ${id}`,
        created_at: '2025-01-01T00:00:00Z',
      },
    ]);

    const { getByTestId, rerender } = render(
      <ThreadHistoryLoader threadId="t1">
        <div>content</div>
      </ThreadHistoryLoader>,
    );

    await waitFor(() => {
      expect(getByTestId('chat-runtime-provider').getAttribute('data-thread-id')).toBe('t1');
    });
    expect(getByTestId('initial-messages-count').textContent).toBe('1');

    rerender(
      <ThreadHistoryLoader threadId="t2">
        <div>content</div>
      </ThreadHistoryLoader>,
    );

    await waitFor(() => {
      expect(getByTestId('chat-runtime-provider').getAttribute('data-thread-id')).toBe('t2');
    });
    expect(getMessagesMock).toHaveBeenCalledTimes(2);
    expect(getMessagesMock).toHaveBeenNthCalledWith(1, 't1');
    expect(getMessagesMock).toHaveBeenNthCalledWith(2, 't2');
  });

  it('旧请求在切换后返回时不应覆盖新 thread 的状态', async () => {
    let resolveT1: (v: unknown[]) => void = () => {};
    getMessagesMock.mockImplementation((id: string) => {
      if (id === 't1') {
        return new Promise<unknown[]>((resolve) => { resolveT1 = resolve; });
      }
      return Promise.resolve([
        { id: `u-${id}`, thread_id: id, role: 'user', content: 'new', created_at: '2025-01-01T00:00:00Z' },
      ]);
    });

    const { getByTestId, rerender } = render(
      <ThreadHistoryLoader threadId="t1">
        <div>content</div>
      </ThreadHistoryLoader>,
    );

    rerender(
      <ThreadHistoryLoader threadId="t2">
        <div>content</div>
      </ThreadHistoryLoader>,
    );

    await waitFor(() => {
      expect(getByTestId('chat-runtime-provider').getAttribute('data-thread-id')).toBe('t2');
    });

    // 现在 t1 旧请求才回来：预期被 cancelled 标记丢弃，不会改写状态
    resolveT1([
      { id: 'u-t1', thread_id: 't1', role: 'user', content: 'stale', created_at: '2025-01-01T00:00:00Z' },
    ]);
    await new Promise((r) => setTimeout(r, 0));

    expect(getByTestId('chat-runtime-provider').getAttribute('data-thread-id')).toBe('t2');
    expect(getByTestId('initial-messages-count').textContent).toBe('1');
  });

  it('将 mapper 的 restoredApprovals seed 到 ApprovalProvider', async () => {
    // 历史中有一条 approval_needed，没有 resolved → mapper 会产出一条 restoredApproval
    const needed = JSON.stringify({
      kind: 'ui_event',
      event: 'approval_needed',
      request_id: 'req-A',
      tool_name: 'bash',
      description: 'run ls',
    });
    getMessagesMock.mockResolvedValue([
      { id: 's1', thread_id: 't1', role: 'system', content: needed, created_at: '2025-01-01T00:00:00Z' },
      { id: 'u1', thread_id: 't1', role: 'user', content: 'hi', created_at: '2025-01-01T00:00:01Z' },
    ]);

    const { getByTestId } = render(
      <ThreadHistoryLoader threadId="t1">
        <div>content</div>
      </ThreadHistoryLoader>,
    );

    await waitFor(() => {
      expect(getByTestId('approval-provider').getAttribute('data-approvals-count')).toBe('1');
    });
    expect(getByTestId('approval-provider').getAttribute('data-thread-id')).toBe('t1');

    const lastCall = approvalProviderMock.mock.calls.at(-1)?.[0] as Record<string, unknown>;
    expect(lastCall?.initialPendingApprovals).toEqual([
      { request_id: 'req-A', tool_name: 'bash', description: 'run ls' },
    ]);
  });
});
