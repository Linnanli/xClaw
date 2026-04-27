/**
 * ThreadHistoryLoader — 在 ChatRuntimeProvider 挂载前预取历史消息 + pending approvals。
 *
 * Phase 1.3.b：红线 #4 落地的关键组件。
 * Phase 1.3.b 补丁：顺带 seed ApprovalProvider 的初始 pending 队列。
 *
 * 背景：`useChatRuntime(adapters.history)` 内部的 `useExternalHistory` 要求
 *   `threadListItem.getState().remoteId` 非空才会触发加载；Desktop 场景下
 *   我们不挂 cloud / threadList，那条通道不可用。
 *
 * 变通方案（本组件）：
 *   1. 在 `useChatRuntime` 挂载之前，异步调用 `threadApi.getMessages(threadId)`
 *      把后端历史拿下来。
 *   2. 用 `mapTauriMessagesToUIMessages` 映射成 AI SDK v5 `UIMessage[]` + 残留
 *      pending approvals（mapper 已处理 `approval_resolved` 冲销）。
 *   3. 把结果分别作为 `initialMessages` / `initialPendingApprovals` 传给
 *      `ChatRuntimeProvider` / `ApprovalProvider`，挂载时一次性注入。
 *   4. `key={threadId}` 强制 thread 切换时整棵 Runtime + Approval 树重建。
 */

import { useEffect, useState, type ReactNode } from 'react';
import { ChatRuntimeProvider, type InitialChatMessage } from './ChatRuntimeProvider';
import { ApprovalProvider } from './contexts/ApprovalProvider';
import {
  mapTauriMessagesToUIMessages,
  type RestoredApproval,
} from './shared/historyLoader';
import { threadApi } from '../utils/tauri';
import { tracing } from '../utils/tracing';

interface ThreadHistoryLoaderProps {
  children: ReactNode;
  threadId: string | null;
  modelId?: string | null;
  apiBaseUrl?: string | null;
  apiKey?: string | null;
}

type LoadState =
  | { status: 'idle' }
  | { status: 'loading' }
  | {
      status: 'ready';
      messages: InitialChatMessage[];
      approvals: RestoredApproval[];
    }
  | { status: 'error'; error: unknown };

const EMPTY_READY: Extract<LoadState, { status: 'ready' }> = {
  status: 'ready',
  messages: [],
  approvals: [],
};

export function ThreadHistoryLoader({
  children,
  threadId,
  modelId,
  apiBaseUrl,
  apiKey,
}: ThreadHistoryLoaderProps) {
  const [state, setState] = useState<LoadState>(
    threadId ? { status: 'idle' } : EMPTY_READY,
  );

  useEffect(() => {
    if (!threadId) {
      setState(EMPTY_READY);
      return;
    }

    let cancelled = false;
    setState({ status: 'loading' });

    threadApi
      .getMessages(threadId)
      .then((history) => {
        if (cancelled) return;
        const { messages, restoredApprovals } = mapTauriMessagesToUIMessages(history);
        setState({
          status: 'ready',
          messages: messages as unknown as InitialChatMessage[],
          approvals: restoredApprovals,
        });
      })
      .catch((error) => {
        if (cancelled) return;
        tracing.error('Failed to load thread history for ChatRuntime', { error, threadId });
        // 失败降级：仍然挂载 Runtime + 空 approvals；用户新发送消息不会因此被阻塞。
        setState({ status: 'error', error });
      });

    return () => {
      cancelled = true;
    };
  }, [threadId]);

  if (state.status === 'loading') {
    return (
      <div
        data-testid="chat-runtime-history-loading"
        className="flex h-full flex-col items-center justify-center bg-background text-sm text-muted-foreground"
      >
        <div className="flex items-center gap-2">
          <span className="h-2 w-2 animate-pulse rounded-full bg-sky-500" aria-hidden />
          加载会话历史…
        </div>
      </div>
    );
  }

  if (state.status === 'idle') {
    // 短暂状态：仅在首帧 threadId 非空时出现；渲染空占位而不阻塞交互
    return <div data-testid="chat-runtime-history-idle" />;
  }

  const initialMessages =
    state.status === 'ready' ? state.messages : ([] as InitialChatMessage[]);
  const initialApprovals =
    state.status === 'ready' ? state.approvals : ([] as RestoredApproval[]);

  return (
    <ApprovalProvider
      // key 确保 threadId 切换时 Approval 树连同新 seed 一起重建
      key={threadId ?? '__empty__'}
      threadId={threadId}
      initialPendingApprovals={initialApprovals}
    >
      <ChatRuntimeProvider
        threadId={threadId}
        modelId={modelId ?? null}
        apiBaseUrl={apiBaseUrl ?? null}
        apiKey={apiKey ?? null}
        initialMessages={initialMessages}
      >
        {children}
      </ChatRuntimeProvider>
    </ApprovalProvider>
  );
}
