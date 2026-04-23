/**
 * ApprovalProvider — Phase 1.2 AI-SDK 新 Runtime 的即时工具授权状态 Provider
 *
 * 职责：
 * - 自行 `listen('chat-stream')`，按 `data-custom.data.type` 过滤
 *   `approval_needed` / `approval_resolved`，维护 `pendingApprovals` 队列。
 * - 向下暴露 `useApprovalState()`（形状与旧 `TauriRuntimeProvider` 兼容），
 *   `FloatingApprovalBanner`（`thread.tsx`）无需改动即可消费。
 * - `approve(requestId)` / `deny(requestId)` 通过 `ic_approve_tool` / `ic_deny_tool`
 *   回传审批决定；threadId 由 props 注入，不再由消费者传参。
 *
 * 事件多订阅者说明：新 Runtime 树中，`TauriChatTransport`、`useEngineReady`、
 * 本 Provider 都各自 `listen('chat-stream')`。Tauri 侧同一事件会对每个订阅者
 * 递送一份，与 `useEngineReady` 现状一致；若后续出现热路径性能问题，
 * 可抽 `ChatStreamBus` 做内部 fan-out。
 *
 * 向后兼容：旧 `TauriRuntimeProvider` 仍拥有自己的 `ApprovalContext.Provider`
 * 包裹其子树；为避免定义双份 context，`ApprovalContext` / `PendingApproval`
 * / `useApprovalState` 集中在本文件定义，由旧 Provider 重新导出。
 */

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { tracing } from '../../utils/tracing';

export interface PendingApproval {
  request_id: string;
  tool_name: string;
  description: string;
}

interface ApprovalState {
  pendingApprovals: PendingApproval[];
  approve: (requestId: string) => Promise<void>;
  deny: (requestId: string) => Promise<void>;
}

export const ApprovalContext = createContext<ApprovalState>({
  pendingApprovals: [],
  approve: async () => {},
  deny: async () => {},
});

export const useApprovalState = (): ApprovalState => useContext(ApprovalContext);

// --- Provider --------------------------------------------------------------

interface ApprovalStreamPayload {
  type?: string;
  data?: {
    type?: string;
    request_id?: string;
    tool_name?: string;
    description?: string;
    thread_id?: string;
  };
}

export interface ApprovalProviderProps {
  /** 当前会话 id；审批决定回传给后端时必须带上。允许 null（无会话时退化成空队列）。 */
  threadId: string | null;
  /**
   * 从历史消息中恢复的 pending approvals，作为初始 seed。
   * 由 `ThreadHistoryLoader` 经 `mapTauriMessagesToUIMessages` 算出；mapper 已处理
   * `approval_resolved` 冲销，所以这里的是 final pending 集合，直接使用即可。
   * threadId 切换时下游 seed 由 key 重建迫使重读，这里仅备注：不要在 threadId
   * 不变的同一挂载周期内重复注入（会覆盖 chat-stream 以来的现场状态）。
   */
  initialPendingApprovals?: readonly PendingApproval[];
  children: ReactNode;
}

export function ApprovalProvider({
  threadId,
  initialPendingApprovals,
  children,
}: ApprovalProviderProps) {
  const [pendingApprovals, setPendingApprovals] = useState<PendingApproval[]>(
    () => (initialPendingApprovals ? [...initialPendingApprovals] : []),
  );
  // ref 保证在 listener 闭包里读到最新 threadId，避免 unlisten/relisten 抖动
  const threadIdRef = useRef<string | null>(threadId);
  threadIdRef.current = threadId;

  // 会话切换：重新 seed。依赖 threadId 变化 → 注入新 seed（或清空）。
  // 本层父组件（ThreadHistoryLoader）在 threadId 切换时会重新 fetch 并传新的
  // initialPendingApprovals，因此 seed 总是与 threadId 匹配。
  useEffect(() => {
    setPendingApprovals(
      initialPendingApprovals ? [...initialPendingApprovals] : [],
    );
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [threadId]);

  useEffect(() => {
    const unlistenPromise = listen<ApprovalStreamPayload>('chat-stream', (event) => {
      const payload = event.payload;
      if (!payload || payload.type !== 'data-custom') return;
      const data = payload.data;
      if (!data || typeof data.type !== 'string') return;

      // 仅处理属于当前 thread 的事件（thread_id 缺省视为"全局"，保留）
      if (data.thread_id && threadIdRef.current && data.thread_id !== threadIdRef.current) {
        return;
      }

      if (data.type === 'approval_needed') {
        const requestId = data.request_id;
        const toolName = data.tool_name;
        if (!requestId || !toolName) return;
        setPendingApprovals((prev) => {
          if (prev.some((a) => a.request_id === requestId)) return prev;
          return [
            ...prev,
            {
              request_id: requestId,
              tool_name: toolName,
              description: data.description ?? '',
            },
          ];
        });
        return;
      }

      if (data.type === 'approval_resolved') {
        const requestId = data.request_id;
        if (!requestId) return;
        setPendingApprovals((prev) => prev.filter((a) => a.request_id !== requestId));
        return;
      }
    });

    return () => {
      void unlistenPromise.then((fn) => fn()).catch(() => {});
    };
  }, []);

  const sendDecision = useCallback(
    async (command: 'ic_approve_tool' | 'ic_deny_tool', requestId: string) => {
      const tid = threadIdRef.current ?? '';
      try {
        await invoke(command, { requestId, threadId: tid });
        setPendingApprovals((prev) => prev.filter((a) => a.request_id !== requestId));
      } catch (err) {
        tracing.error(`ApprovalProvider: ${command} failed`, { requestId, error: err });
        throw err;
      }
    },
    [],
  );

  const approve = useCallback(
    (requestId: string) => sendDecision('ic_approve_tool', requestId),
    [sendDecision],
  );
  const deny = useCallback(
    (requestId: string) => sendDecision('ic_deny_tool', requestId),
    [sendDecision],
  );

  const value = useMemo(
    () => ({ pendingApprovals, approve, deny }),
    [pendingApprovals, approve, deny],
  );

  return <ApprovalContext.Provider value={value}>{children}</ApprovalContext.Provider>;
}
