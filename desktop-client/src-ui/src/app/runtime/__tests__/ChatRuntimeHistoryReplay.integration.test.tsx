/**
 * ChatRuntime history replay 契约测试（Phase 1.3.c）
 *
 * 目标：在真实 `useChatRuntime` + 真实 `AssistantRuntimeProvider` 的环境下，
 * 验证 `initialMessages` 能够被注入 assistant-ui 的 thread state。
 *
 * 覆盖：
 * - happy path：两条 UIMessage 作为 seed → `runtime.thread.getState().messages.length === 2`
 * - 空 seed：messages.length === 0，runtime 仍可挂载
 * - thread 切换（key 重建）：新 seed 生效，旧 seed 不残留
 *
 * 与 `ChatRuntimeProvider.test.tsx` 的差别：
 * - 该文件 mock 了 `useChatRuntime`，只验证参数传递
 * - 本文件**不 mock `useChatRuntime`**，验证 seed 真的进了 assistant-ui 状态
 *
 * 注：Thread 组件依赖较重，此处用 Probe 组件直接读 runtime 状态，
 * 不渲染 Thread；UI 层面的显示交由 E2E（Step H）覆盖。
 */

import { render, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useAssistantRuntime } from '@assistant-ui/react';

// Mock 所有 Tauri 侧调用：transport 构造不应触发真实 IPC
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn().mockResolvedValue(undefined) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));

// 工具 renderer 都是空实现，避免 makeAssistantToolUI 的副作用
vi.mock('../../components/assistant-ui/tool-renderers/echo-renderer', () => ({ EchoToolUI: () => null }));
vi.mock('../../components/assistant-ui/tool-renderers/file-edit-renderer', () => ({ FileEditToolUI: () => null }));
vi.mock('../../components/assistant-ui/tool-renderers/grep-result-renderer', () => ({ GrepResultToolUI: () => null }));
vi.mock('../../components/assistant-ui/tool-renderers/shell-output-renderer', () => ({ ShellOutputToolUI: () => null }));
vi.mock('../../components/assistant-ui/tool-renderers/glob-result-renderer', () => ({ GlobResultToolUI: () => null }));
vi.mock('../../components/assistant-ui/tool-renderers/git-diff-renderer', () => ({ GitDiffToolUI: () => null }));
vi.mock('../../components/assistant-ui/tool-renderers/lsp-result-renderer', () => ({ LspResultToolUI: () => null }));
vi.mock('../../components/assistant-ui/tool-renderers/plan-renderer', () => ({ PlanModeToolUI: () => null }));
vi.mock('../../components/assistant-ui/tool-renderers/fork-renderer', () => ({ SessionForkToolUI: () => null }));
vi.mock('../../components/assistant-ui/tool-renderers/sub-agent-renderer', () => ({ SubAgentToolUI: () => null }));
vi.mock('../../components/tool-ui/approval-tool-ui', () => ({
  ApprovalToolUI: () => null,
  ApprovalThreadIdProvider: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

import { ChatRuntimeProvider, type InitialChatMessage } from '../ChatRuntimeProvider';

interface ProbeResult {
  count: number;
  roles: string[];
}

function Probe({ onResult }: { onResult: (r: ProbeResult) => void }) {
  const runtime = useAssistantRuntime();
  const state = runtime.thread.getState();
  onResult({
    count: state.messages.length,
    roles: state.messages.map((m) => m.role),
  });
  return <div data-testid="probe-mounted" />;
}

const seedMessages: InitialChatMessage[] = [
  {
    id: 'u1',
    role: 'user',
    parts: [{ type: 'text', text: '你好', state: 'done' }],
  },
  {
    id: 'a1',
    role: 'assistant',
    parts: [{ type: 'text', text: '你好，有什么可以帮你的？', state: 'done' }],
  },
];

describe('ChatRuntime history replay (Phase 1.3.c 集成)', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('initialMessages 被注入后 runtime.thread 中包含对应 messages', async () => {
    let latest: ProbeResult | null = null;
    const onResult = (r: ProbeResult) => { latest = r; };

    render(
      <ChatRuntimeProvider threadId="t-1" initialMessages={seedMessages}>
        <Probe onResult={onResult} />
      </ChatRuntimeProvider>,
    );

    await waitFor(() => {
      expect(latest).not.toBeNull();
      expect(latest!.count).toBe(2);
    });
    expect(latest!.roles).toEqual(['user', 'assistant']);
  });

  it('空 initialMessages 时 runtime 仍可挂载，messages 为 0', async () => {
    let latest: ProbeResult | null = null;

    render(
      <ChatRuntimeProvider threadId="t-empty">
        <Probe onResult={(r) => { latest = r; }} />
      </ChatRuntimeProvider>,
    );

    await waitFor(() => {
      expect(latest).not.toBeNull();
    });
    expect(latest!.count).toBe(0);
  });

  it('threadId 切换（外层 key 重建）时新 seed 完全覆盖旧 seed', async () => {
    const results: ProbeResult[] = [];
    const onResult = (r: ProbeResult) => { results.push(r); };

    const { rerender } = render(
      <ChatRuntimeProvider key="t-1" threadId="t-1" initialMessages={seedMessages}>
        <Probe onResult={onResult} />
      </ChatRuntimeProvider>,
    );

    await waitFor(() => {
      expect(results.at(-1)?.count).toBe(2);
    });

    const newSeed: InitialChatMessage[] = [
      { id: 'u2', role: 'user', parts: [{ type: 'text', text: '新会话', state: 'done' }] },
    ];

    // 模拟 ThreadHistoryLoader 用 key 重建：rerender 时换 key
    rerender(
      <ChatRuntimeProvider key="t-2" threadId="t-2" initialMessages={newSeed}>
        <Probe onResult={onResult} />
      </ChatRuntimeProvider>,
    );

    await waitFor(() => {
      const last = results.at(-1);
      expect(last?.count).toBe(1);
      expect(last?.roles).toEqual(['user']);
    });
  });
});
