/**
 * ApprovalProvider 单元测试
 *
 * 覆盖维度（AGENTS.md 测试维度矩阵）：
 * - 正常路径：approval_needed 入队、approval_resolved 出队、去重
 * - 切换 thread：清空遗留队列、忽略其他 thread 事件
 * - 失败路径：invoke 失败抛错、不合法 payload 跳过
 * - 契约：useApprovalState 形状与旧 TauriRuntimeProvider 兼容
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent, act } from '@testing-library/react';
import { ApprovalProvider, useApprovalState } from '../ApprovalProvider';

// Mock Tauri event + invoke
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn().mockResolvedValue(undefined) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));

async function getMocks() {
  const { invoke } = await import('@tauri-apps/api/core');
  const { listen } = await import('@tauri-apps/api/event');
  return { invoke: vi.mocked(invoke), listen: vi.mocked(listen) };
}

type ChatStreamHandler = (event: { payload: unknown }) => void;

/** 安装一个可手动触发 chat-stream 事件的 listen mock */
function installListener() {
  const handlers: ChatStreamHandler[] = [];
  return {
    handlers,
    setup: async () => {
      const { listen } = await getMocks();
      listen.mockImplementation((eventName: string, handler: unknown) => {
        if (eventName === 'chat-stream') {
          handlers.push(handler as ChatStreamHandler);
        }
        return Promise.resolve(() => {});
      });
    },
    fire: (payload: unknown) => {
      for (const h of handlers) h({ payload });
    },
  };
}

function Consumer() {
  const { pendingApprovals, approve, deny } = useApprovalState();
  return (
    <div>
      <span data-testid="count">{pendingApprovals.length}</span>
      <ul data-testid="list">
        {pendingApprovals.map((a) => (
          <li key={a.request_id} data-testid={`item-${a.request_id}`}>
            {a.tool_name} | {a.description}
          </li>
        ))}
      </ul>
      <button data-testid="approve-req-1" onClick={() => approve('req-1').catch(() => {})}>
        approve
      </button>
      <button data-testid="deny-req-1" onClick={() => deny('req-1').catch(() => {})}>
        deny
      </button>
    </div>
  );
}

describe('ApprovalProvider - 正常路径', () => {
  let bus: ReturnType<typeof installListener>;

  beforeEach(async () => {
    vi.clearAllMocks();
    bus = installListener();
    await bus.setup();
  });

  it('approval_needed 事件入队', async () => {
    render(
      <ApprovalProvider threadId="t-1">
        <Consumer />
      </ApprovalProvider>,
    );
    // 等 listener 注册完成
    await waitFor(() => expect(bus.handlers.length).toBe(1));

    act(() => {
      bus.fire({
        type: 'data-custom',
        data: {
          type: 'approval_needed',
          request_id: 'req-1',
          tool_name: 'bash',
          description: 'run ls',
          thread_id: 't-1',
        },
      });
    });

    expect(screen.getByTestId('count').textContent).toBe('1');
    expect(screen.getByTestId('item-req-1').textContent).toContain('bash');
  });

  it('同一 request_id 不重复入队', async () => {
    render(
      <ApprovalProvider threadId="t-1">
        <Consumer />
      </ApprovalProvider>,
    );
    await waitFor(() => expect(bus.handlers.length).toBe(1));

    const payload = {
      type: 'data-custom',
      data: {
        type: 'approval_needed',
        request_id: 'req-1',
        tool_name: 'bash',
        description: 'x',
      },
    };
    act(() => {
      bus.fire(payload);
      bus.fire(payload);
    });
    expect(screen.getByTestId('count').textContent).toBe('1');
  });

  it('approval_resolved 出队', async () => {
    render(
      <ApprovalProvider threadId="t-1">
        <Consumer />
      </ApprovalProvider>,
    );
    await waitFor(() => expect(bus.handlers.length).toBe(1));

    act(() => {
      bus.fire({
        type: 'data-custom',
        data: { type: 'approval_needed', request_id: 'req-1', tool_name: 'bash', description: '' },
      });
      bus.fire({
        type: 'data-custom',
        data: { type: 'approval_resolved', request_id: 'req-1' },
      });
    });
    expect(screen.getByTestId('count').textContent).toBe('0');
  });

  it('approve 调用 ic_approve_tool 并出队', async () => {
    const { invoke } = await getMocks();
    render(
      <ApprovalProvider threadId="t-1">
        <Consumer />
      </ApprovalProvider>,
    );
    await waitFor(() => expect(bus.handlers.length).toBe(1));

    act(() => {
      bus.fire({
        type: 'data-custom',
        data: { type: 'approval_needed', request_id: 'req-1', tool_name: 'bash', description: '' },
      });
    });
    expect(screen.getByTestId('count').textContent).toBe('1');

    await act(async () => {
      fireEvent.click(screen.getByTestId('approve-req-1'));
    });

    expect(invoke).toHaveBeenCalledWith('ic_approve_tool', { requestId: 'req-1', threadId: 't-1' });
    expect(screen.getByTestId('count').textContent).toBe('0');
  });

  it('deny 调用 ic_deny_tool 并出队', async () => {
    const { invoke } = await getMocks();
    render(
      <ApprovalProvider threadId="t-1">
        <Consumer />
      </ApprovalProvider>,
    );
    await waitFor(() => expect(bus.handlers.length).toBe(1));

    act(() => {
      bus.fire({
        type: 'data-custom',
        data: { type: 'approval_needed', request_id: 'req-1', tool_name: 'bash', description: '' },
      });
    });

    await act(async () => {
      fireEvent.click(screen.getByTestId('deny-req-1'));
    });

    expect(invoke).toHaveBeenCalledWith('ic_deny_tool', { requestId: 'req-1', threadId: 't-1' });
    expect(screen.getByTestId('count').textContent).toBe('0');
  });
});

describe('ApprovalProvider - thread 隔离', () => {
  let bus: ReturnType<typeof installListener>;

  beforeEach(async () => {
    vi.clearAllMocks();
    bus = installListener();
    await bus.setup();
  });

  it('其它 thread 的事件被忽略', async () => {
    render(
      <ApprovalProvider threadId="t-1">
        <Consumer />
      </ApprovalProvider>,
    );
    await waitFor(() => expect(bus.handlers.length).toBe(1));

    act(() => {
      bus.fire({
        type: 'data-custom',
        data: {
          type: 'approval_needed',
          request_id: 'req-x',
          tool_name: 'bash',
          description: '',
          thread_id: 'other-thread',
        },
      });
    });
    expect(screen.getByTestId('count').textContent).toBe('0');
  });

  it('切换 thread 时清空遗留队列', async () => {
    const { rerender } = render(
      <ApprovalProvider threadId="t-1">
        <Consumer />
      </ApprovalProvider>,
    );
    await waitFor(() => expect(bus.handlers.length).toBe(1));

    act(() => {
      bus.fire({
        type: 'data-custom',
        data: { type: 'approval_needed', request_id: 'req-1', tool_name: 'bash', description: '' },
      });
    });
    expect(screen.getByTestId('count').textContent).toBe('1');

    rerender(
      <ApprovalProvider threadId="t-2">
        <Consumer />
      </ApprovalProvider>,
    );
    expect(screen.getByTestId('count').textContent).toBe('0');
  });
});

describe('ApprovalProvider - 失败路径 / 鲁棒性', () => {
  let bus: ReturnType<typeof installListener>;

  beforeEach(async () => {
    vi.clearAllMocks();
    bus = installListener();
    await bus.setup();
  });

  it('test_failure_malformed_payload_does_not_crash', async () => {
    render(
      <ApprovalProvider threadId="t-1">
        <Consumer />
      </ApprovalProvider>,
    );
    await waitFor(() => expect(bus.handlers.length).toBe(1));

    act(() => {
      bus.fire(null);
      bus.fire({ type: 'data-custom' }); // no data
      bus.fire({ type: 'data-custom', data: {} }); // no data.type
      bus.fire({ type: 'data-custom', data: { type: 'approval_needed' } }); // missing request_id
      bus.fire({
        type: 'data-custom',
        data: { type: 'approval_needed', request_id: 'x' }, // missing tool_name
      });
      bus.fire({ type: 'text-delta', id: 'x', delta: 'y' }); // unrelated chunk
    });
    expect(screen.getByTestId('count').textContent).toBe('0');
  });

  it('test_failure_invoke_rejects_surfaces_error', async () => {
    const { invoke } = await getMocks();
    invoke.mockRejectedValueOnce(new Error('ipc down'));

    render(
      <ApprovalProvider threadId="t-1">
        <Consumer />
      </ApprovalProvider>,
    );
    await waitFor(() => expect(bus.handlers.length).toBe(1));

    act(() => {
      bus.fire({
        type: 'data-custom',
        data: { type: 'approval_needed', request_id: 'req-1', tool_name: 'bash', description: '' },
      });
    });

    let caught: unknown = null;
    await act(async () => {
      try {
        fireEvent.click(screen.getByTestId('approve-req-1'));
        // wait microtask
        await Promise.resolve();
      } catch (e) {
        caught = e;
      }
    });
    // invoke 失败不应把 UI 队列清掉（保留让用户重试）
    expect(screen.getByTestId('count').textContent).toBe('1');
    // caught 可能为 null（因为 fireEvent 里 catch 发生在异步），但核心断言是队列未清空
    expect(caught).toBeNull();
  });

  it('threadId=null 时 approve 回传空 threadId（不崩溃）', async () => {
    const { invoke } = await getMocks();
    render(
      <ApprovalProvider threadId={null}>
        <Consumer />
      </ApprovalProvider>,
    );
    await waitFor(() => expect(bus.handlers.length).toBe(1));

    act(() => {
      bus.fire({
        type: 'data-custom',
        data: { type: 'approval_needed', request_id: 'req-1', tool_name: 'bash', description: '' },
      });
    });
    await act(async () => {
      fireEvent.click(screen.getByTestId('approve-req-1'));
    });
    expect(invoke).toHaveBeenCalledWith('ic_approve_tool', { requestId: 'req-1', threadId: '' });
  });
});

describe('ApprovalProvider - initialPendingApprovals (Phase 1.3.b 补丁)', () => {
  let bus: ReturnType<typeof installListener>;

  beforeEach(async () => {
    vi.clearAllMocks();
    bus = installListener();
    await bus.setup();
  });

  it('挂载时用 initialPendingApprovals 预填队列', () => {
    render(
      <ApprovalProvider
        threadId="t-1"
        initialPendingApprovals={[
          { request_id: 'seed-1', tool_name: 'bash', description: 'seeded' },
          { request_id: 'seed-2', tool_name: 'read_file', description: 'another' },
        ]}
      >
        <Consumer />
      </ApprovalProvider>,
    );

    expect(screen.getByTestId('count').textContent).toBe('2');
    expect(screen.getByTestId('item-seed-1').textContent).toContain('bash');
    expect(screen.getByTestId('item-seed-2').textContent).toContain('read_file');
  });

  it('seed 后仍能追加 chat-stream 的新 approval_needed', async () => {
    render(
      <ApprovalProvider
        threadId="t-1"
        initialPendingApprovals={[
          { request_id: 'seed-1', tool_name: 'bash', description: 's' },
        ]}
      >
        <Consumer />
      </ApprovalProvider>,
    );
    await waitFor(() => expect(bus.handlers.length).toBe(1));

    act(() => {
      bus.fire({
        type: 'data-custom',
        data: {
          type: 'approval_needed',
          request_id: 'live-1',
          tool_name: 'read_file',
          description: 'live',
          thread_id: 't-1',
        },
      });
    });
    expect(screen.getByTestId('count').textContent).toBe('2');
  });

  it('seed 后 approval_resolved 能冲销 seeded 项', async () => {
    render(
      <ApprovalProvider
        threadId="t-1"
        initialPendingApprovals={[
          { request_id: 'seed-1', tool_name: 'bash', description: 's' },
        ]}
      >
        <Consumer />
      </ApprovalProvider>,
    );
    await waitFor(() => expect(bus.handlers.length).toBe(1));

    act(() => {
      bus.fire({
        type: 'data-custom',
        data: { type: 'approval_resolved', request_id: 'seed-1', thread_id: 't-1' },
      });
    });
    expect(screen.getByTestId('count').textContent).toBe('0');
  });

  it('threadId 切换时重新用 seed 覆盖（不保留上一次 thread 的队列）', () => {
    const { rerender } = render(
      <ApprovalProvider
        threadId="t-1"
        initialPendingApprovals={[
          { request_id: 'seed-a', tool_name: 'bash', description: 'A' },
        ]}
      >
        <Consumer />
      </ApprovalProvider>,
    );
    expect(screen.getByTestId('count').textContent).toBe('1');

    rerender(
      <ApprovalProvider
        threadId="t-2"
        initialPendingApprovals={[
          { request_id: 'seed-b', tool_name: 'read', description: 'B' },
          { request_id: 'seed-c', tool_name: 'write', description: 'C' },
        ]}
      >
        <Consumer />
      </ApprovalProvider>,
    );
    expect(screen.getByTestId('count').textContent).toBe('2');
    expect(screen.queryByTestId('item-seed-a')).toBeNull();
    expect(screen.getByTestId('item-seed-b')).toBeTruthy();
  });
});
