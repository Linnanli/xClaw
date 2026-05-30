/**
 * ApprovalToolUI — 基于 assistant-ui `makeAssistantToolUI` 的审批卡片
 *
 * 架构定位（Phase 1 Step F）：
 * - 后端（Step G）把 `StatusUpdate::ApprovalNeeded` 建模为"虚拟工具调用"，
 *   通过 `tool-input-available` 帧发送（toolName = "approval_request"）
 * - 本组件订阅该 toolName，SDK 自动按 `toolCallId` 挂载到正确 branch，
 *   彻底替代旧的 `FloatingApprovalBanner`（临时 hack）
 * - 审批响应走 `ic_approve_tool` / `ic_deny_tool` 命令（与旧通路保持兼容）
 * - 后端收到审批结果后发 `tool-output-available`，SDK 自动把 status 切到 complete
 *
 * 设计：
 * - thread_id 通过 `ApprovalThreadIdContext` 注入（由 ChatRuntimeProvider 提供）
 * - args.parameters 仅在 description 中展示摘要，**不直接渲染原始 JSON**，防敏感信息泄露
 *   （与后端 `tauri_channel::sanitize` 对齐）
 */

import { createContext, useCallback, useContext, type ReactNode } from 'react';
import { makeAssistantToolUI } from '@assistant-ui/react';
import { invoke } from '@tauri-apps/api/core';
import { CheckIcon, ShieldAlert, XCircleIcon } from 'lucide-react';

// ============================================================================
// 工具参数 / 结果类型（与后端 Step G 协议对齐）
// ============================================================================

export interface ApprovalArgs {
  request_id: string;
  tool_name: string;
  description: string;
  /** 后端 sanitize 后的参数摘要；前端不展示原文 */
  parameters?: Record<string, unknown>;
  allow_always?: boolean;
}

export interface ApprovalResult {
  approved: boolean;
}

// ============================================================================
// ThreadId 注入：由 ChatRuntimeProvider 在顶层提供
// ============================================================================

const ApprovalThreadIdContext = createContext<string | null>(null);

export const ApprovalThreadIdProvider = ({
  threadId,
  children,
}: {
  threadId: string | null;
  children: ReactNode;
}) => (
  <ApprovalThreadIdContext.Provider value={threadId}>{children}</ApprovalThreadIdContext.Provider>
);

export const useApprovalThreadId = () => useContext(ApprovalThreadIdContext);

// ============================================================================
// 视图：与旧 ApprovalCard 外观一致
// ============================================================================

interface ApprovalCardViewProps {
  toolName: string;
  description: string;
  disabled: boolean;
  onApprove: () => void;
  onDeny: () => void;
}

const ApprovalCardView = ({
  toolName,
  description,
  disabled,
  onApprove,
  onDeny,
}: ApprovalCardViewProps) => (
  <div
    data-testid="approval-card"
    className="flex items-start gap-3 rounded-xl border border-amber-200 bg-amber-50 p-3 dark:border-amber-800/40 dark:bg-amber-950/20"
  >
    <ShieldAlert className="mt-0.5 size-4 shrink-0 text-amber-500" />
    <div className="flex flex-1 flex-col gap-2">
      <div>
        <p className="text-xs font-semibold text-foreground">
          工具执行需要授权：
          <code className="rounded bg-amber-100 px-1 dark:bg-amber-900/40">{toolName}</code>
        </p>
        {description && (
          <p className="mt-0.5 text-xs text-muted-foreground">{description}</p>
        )}
      </div>
      <div className="flex gap-2">
        <button
          type="button"
          data-testid="approval-approve"
          disabled={disabled}
          onClick={onApprove}
          className="flex items-center gap-1 rounded-md bg-primary px-3 py-1 text-xs font-medium text-primary-foreground transition-opacity hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-50"
        >
          <CheckIcon className="size-3" />
          批准
        </button>
        <button
          type="button"
          data-testid="approval-deny"
          disabled={disabled}
          onClick={onDeny}
          className="flex items-center gap-1 rounded-md border border-border px-3 py-1 text-xs font-medium text-muted-foreground transition-colors hover:bg-destructive/10 hover:text-destructive disabled:cursor-not-allowed disabled:opacity-50"
        >
          <XCircleIcon className="size-3" />
          拒绝
        </button>
      </div>
    </div>
  </div>
);

const ApprovalResultBanner = ({ approved }: { approved: boolean }) => (
  <div
    data-testid="approval-result"
    data-approved={approved}
    className={`rounded-xl border px-3 py-2 text-xs ${
      approved
        ? 'border-emerald-200 bg-emerald-50 text-emerald-800 dark:border-emerald-800/40 dark:bg-emerald-950/20 dark:text-emerald-300'
        : 'border-rose-200 bg-rose-50 text-rose-800 dark:border-rose-800/40 dark:bg-rose-950/20 dark:text-rose-300'
    }`}
  >
    {approved ? '已批准工具执行' : '已拒绝工具执行'}
  </div>
);

// ============================================================================
// ToolUI 注册组件
// ============================================================================

/** 暴露一个可复用的 render 函数，便于单元测试直接渲染。 */
export const ApprovalToolRenderer = ({
  args,
  status,
  result,
  addResult,
}: {
  args: ApprovalArgs;
  status: { type: 'running' | 'complete' | 'incomplete' | 'requires-action' };
  result?: ApprovalResult;
  /** SDK 提供的本地结果注入钩子（makeAssistantToolUI render props 里原生自带） */
  addResult?: (result: ApprovalResult) => void;
}) => {
  const threadId = useApprovalThreadId();

  const respond = useCallback(
    async (approved: boolean) => {
      if (!args.request_id) return;
      const command = approved ? 'ic_approve_tool' : 'ic_deny_tool';
      await invoke(command, {
        requestId: args.request_id,
        threadId: threadId ?? '',
      });
      // 乐观把 ToolCall 推进到 complete；SDK 据此自动隐藏按钮
      addResult?.({ approved });
    },
    [args.request_id, threadId, addResult],
  );

  const handleApprove = useCallback(() => respond(true), [respond]);
  const handleDeny = useCallback(() => respond(false), [respond]);

  if (status.type === 'complete') {
    return <ApprovalResultBanner approved={result?.approved ?? false} />;
  }

  if (status.type === 'running' || status.type === 'requires-action') {
    return (
      <ApprovalCardView
        toolName={args.tool_name}
        description={args.description}
        disabled={!threadId}
        onApprove={handleApprove}
        onDeny={handleDeny}
      />
    );
  }

  return null;
};

/**
 * 挂载到 AssistantRuntimeProvider 的 ToolUI 注册器。
 * 在 ChatRuntimeProvider 内部渲染一次即可。
 */
export const ApprovalToolUI = makeAssistantToolUI<ApprovalArgs, ApprovalResult>({
  toolName: 'approval_request',
  render: ({ args, status, result, addResult }) => (
    <ApprovalToolRenderer
      args={args}
      status={status}
      result={result}
      addResult={addResult as (r: ApprovalResult) => void}
    />
  ),
});
