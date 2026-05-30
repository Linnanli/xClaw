/**
 * ApprovalToolUI 单元测试
 *
 * 覆盖维度：
 * - 正常路径：running → 显示卡片；批准按钮调用 ic_approve_tool
 * - 正常路径：complete → 显示结果 banner
 * - 失败路径：threadId 为 null 时按钮 disabled（fail-safe）
 * - 契约测试：invoke 参数名 camelCase，匹配后端 #[tauri::command]
 * - 安全审计：UI 不直接渲染 args.parameters 原文
 */

import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn().mockResolvedValue(undefined),
}));

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

// ToolUI 注册 hook 在测试里用不到；直接 stub 掉
vi.mock('@assistant-ui/react', () => ({
  makeAssistantToolUI: () => () => null,
}));

import {
  ApprovalToolRenderer,
  ApprovalThreadIdProvider,
  type ApprovalArgs,
} from '../approval-tool-ui';

const baseArgs: ApprovalArgs = {
  request_id: '550e8400-e29b-41d4-a716-446655440000',
  tool_name: 'shell',
  description: '执行 rm -rf /tmp/test',
  parameters: { command: 'rm -rf /tmp/test', secret: 'do-not-render' },
  allow_always: false,
};

const renderAt = (
  threadId: string | null,
  args: ApprovalArgs,
  status: { type: 'running' | 'complete' | 'incomplete' | 'requires-action' },
  result?: { approved: boolean },
  addResult?: (r: { approved: boolean }) => void,
) =>
  render(
    <ApprovalThreadIdProvider threadId={threadId}>
      <ApprovalToolRenderer
        args={args}
        status={status}
        result={result}
        addResult={addResult}
      />
    </ApprovalThreadIdProvider>,
  );

describe('ApprovalToolUI', () => {
  beforeEach(() => {
    invokeMock.mockClear();
    invokeMock.mockResolvedValue(undefined);
  });

  it('renders approval card on running status with tool name and description', () => {
    renderAt('thread-1', baseArgs, { type: 'running' });
    expect(screen.getByTestId('approval-card')).toBeTruthy();
    expect(screen.getByText(/shell/)).toBeTruthy();
    expect(screen.getByText(/rm -rf \/tmp\/test/)).toBeTruthy();
  });

  it('calls ic_approve_tool with camelCase args when approve is clicked', async () => {
    const addResult = vi.fn();
    renderAt('thread-1', baseArgs, { type: 'running' }, undefined, addResult);
    fireEvent.click(screen.getByTestId('approval-approve'));
    expect(invokeMock).toHaveBeenCalledWith('ic_approve_tool', {
      requestId: baseArgs.request_id,
      threadId: 'thread-1',
    });
    // 等待 microtask flush
    await Promise.resolve();
    expect(addResult).toHaveBeenCalledWith({ approved: true });
  });

  it('calls ic_deny_tool with camelCase args when deny is clicked', async () => {
    const addResult = vi.fn();
    renderAt('thread-1', baseArgs, { type: 'running' }, undefined, addResult);
    fireEvent.click(screen.getByTestId('approval-deny'));
    expect(invokeMock).toHaveBeenCalledWith('ic_deny_tool', {
      requestId: baseArgs.request_id,
      threadId: 'thread-1',
    });
    await Promise.resolve();
    expect(addResult).toHaveBeenCalledWith({ approved: false });
  });

  it('renders approved result banner on complete + approved=true', () => {
    renderAt('thread-1', baseArgs, { type: 'complete' }, { approved: true });
    const banner = screen.getByTestId('approval-result');
    expect(banner.getAttribute('data-approved')).toBe('true');
    expect(banner.textContent).toContain('已批准');
  });

  it('renders denied result banner on complete + approved=false', () => {
    renderAt('thread-1', baseArgs, { type: 'complete' }, { approved: false });
    const banner = screen.getByTestId('approval-result');
    expect(banner.getAttribute('data-approved')).toBe('false');
    expect(banner.textContent).toContain('已拒绝');
  });

  it('disables buttons when threadId is missing (fail-safe)', () => {
    renderAt(null, baseArgs, { type: 'running' });
    const approve = screen.getByTestId('approval-approve') as HTMLButtonElement;
    const deny = screen.getByTestId('approval-deny') as HTMLButtonElement;
    expect(approve.disabled).toBe(true);
    expect(deny.disabled).toBe(true);
  });

  it('returns null on incomplete status', () => {
    const { container } = renderAt('thread-1', baseArgs, { type: 'incomplete' });
    expect(container.firstChild).toBeNull();
  });

  it('does not leak raw args.parameters into the DOM', () => {
    const { container } = renderAt('thread-1', baseArgs, { type: 'running' });
    // parameters.secret 值不应出现在渲染结果里
    expect(container.textContent).not.toContain('do-not-render');
    expect(container.innerHTML).not.toContain('do-not-render');
  });

  it('also renders card on requires-action (SDK interrupt) status', () => {
    renderAt('thread-1', baseArgs, { type: 'requires-action' });
    expect(screen.getByTestId('approval-card')).toBeTruthy();
  });
});
