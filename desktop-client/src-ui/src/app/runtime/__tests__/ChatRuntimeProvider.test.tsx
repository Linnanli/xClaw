/**
 * ChatRuntimeProvider 集成测试（新版）
 *
 * 覆盖：
 * 1. Provider 正确渲染 children
 * 2. useChatRuntime 被调用且 transport 是 TauriChatTransport 实例
 * 3. useDlpState 默认值契约
 * 4. 多次 re-render 复用同一 transport 实例（ref 模式）
 */

import { render } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ReactNode } from 'react';

const { useChatRuntimeMock } = vi.hoisted(() => ({
  useChatRuntimeMock: vi.fn(() => ({ __mock: 'runtime' })),
}));

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn().mockResolvedValue(undefined) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));
vi.mock('@assistant-ui/react', () => ({
  AssistantRuntimeProvider: ({ children }: { children: ReactNode }) => <>{children}</>,
  makeAssistantToolUI: () => () => null,
}));
vi.mock('@assistant-ui/react-ai-sdk', () => ({
  useChatRuntime: useChatRuntimeMock,
}));

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

import { ChatRuntimeProvider, useDlpState } from '../ChatRuntimeProvider';
import { TauriChatTransport } from '../TauriChatTransport';

describe('ChatRuntimeProvider', () => {
  beforeEach(() => {
    useChatRuntimeMock.mockClear();
    useChatRuntimeMock.mockReturnValue({ __mock: 'runtime' });
  });

  it('renders children and wires useChatRuntime with a TauriChatTransport', () => {
    const { getByText } = render(
      <ChatRuntimeProvider threadId="thread-1" modelId="gpt-4">
        <div>hello</div>
      </ChatRuntimeProvider>,
    );

    expect(getByText('hello')).toBeTruthy();
    expect(useChatRuntimeMock).toHaveBeenCalled();
    const options = useChatRuntimeMock.mock.calls[0]?.[0];
    expect(options?.transport).toBeInstanceOf(TauriChatTransport);
  });

  it('exposes default DLP state (not blocked, no reason)', () => {
    let captured: ReturnType<typeof useDlpState> | null = null;
    function Probe() {
      captured = useDlpState();
      return null;
    }
    render(
      <ChatRuntimeProvider threadId={null}>
        <Probe />
      </ChatRuntimeProvider>,
    );
    expect(captured).not.toBeNull();
    expect(captured!.blocked).toBe(false);
    expect(captured!.blockReason).toBeNull();
    expect(captured!.redactedStats).toBeNull();
    expect(typeof captured!.onBlocked).toBe('function');
    expect(typeof captured!.onRedacted).toBe('function');
  });

  it('reuses the same transport instance across re-renders', () => {
    const { rerender } = render(
      <ChatRuntimeProvider threadId="t1" modelId="gpt-4">
        <div />
      </ChatRuntimeProvider>,
    );
    const transport1 = useChatRuntimeMock.mock.calls[0]?.[0]?.transport;

    rerender(
      <ChatRuntimeProvider threadId="t1" modelId="gpt-5">
        <div />
      </ChatRuntimeProvider>,
    );
    const lastCall = useChatRuntimeMock.mock.calls[useChatRuntimeMock.mock.calls.length - 1];
    const transport2 = lastCall?.[0]?.transport;
    expect(transport1).toBe(transport2);
  });
});
