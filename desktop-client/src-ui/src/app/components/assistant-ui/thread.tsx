import {
  ComposerAddAttachment,
  ComposerAttachments,
  UserMessageAttachments,
} from "@/app/components/assistant-ui/attachment";
import { MarkdownText } from "@/app/components/assistant-ui/markdown-text";
import { Reasoning, ReasoningGroup } from "@/app/components/assistant-ui/reasoning";
import { ToolFallback } from "@/app/components/assistant-ui/tool-fallback";
import { ToolStepIndicator } from "@/app/components/assistant-ui/tool-step-indicator";
import { ContextToolGroup, groupToolSteps } from "@/app/components/assistant-ui/context-tool-group";
import { TooltipIconButton } from "@/app/components/assistant-ui/tooltip-icon-button";
import { Button } from "@/app/components/ui/button";
import { cn } from "@/app/components/ui/utils";
import { ModelSelector } from "@/app/components/ai/ModelSelector";
import {
  ActionBarMorePrimitive,
  ActionBarPrimitive,
  AuiIf,
  BranchPickerPrimitive,
  ComposerPrimitive,
  ErrorPrimitive,
  MessagePrimitive,
  SuggestionPrimitive,
  ThreadPrimitive,
  useAuiState,
  useComposerRuntime,
} from "@assistant-ui/react";
import type { SanitizationStats } from "@/app/hooks/useDlpScan";
import { useWatermark } from "@/app/hooks/useWatermark";
import { useApprovalState, type PendingApproval, type ToolStep } from "@/app/runtime/TauriRuntimeProvider";
import {
  ArrowDownIcon,
  ArrowUpIcon,
  CheckIcon,
  ChevronLeftIcon,
  ChevronRightIcon,
  CopyIcon,
  DownloadIcon,
  MoreHorizontalIcon,
  PencilIcon,
  RefreshCwIcon,
  ShieldCheck,
  SquareIcon,
  SettingsIcon,
  XCircleIcon,
  ShieldAlert,
} from "lucide-react";
import { type FC, useCallback } from "react";

// ── 类型守卫：从消息 metadata.custom 安全提取 dlpStats ──
function extractDlpStats(custom: unknown): SanitizationStats | undefined {
  if (custom && typeof custom === 'object' && 'dlpStats' in custom) {
    return (custom as { dlpStats: SanitizationStats }).dlpStats;
  }
  return undefined;
}

function extractRoutineTriggerCount(custom: unknown): number {
  if (custom && typeof custom === 'object' && 'routineTriggerCount' in custom) {
    const value = (custom as { routineTriggerCount?: unknown }).routineTriggerCount;
    return typeof value === 'number' && value > 0 ? value : 0;
  }
  return 0;
}

function extractToolSteps(custom: unknown): ToolStep[] | undefined {
  if (custom && typeof custom === 'object' && 'toolSteps' in custom) {
    const steps = (custom as { toolSteps?: unknown }).toolSteps;
    return Array.isArray(steps) ? (steps as ToolStep[]) : undefined;
  }
  return undefined;
}

export const Thread: FC = () => {
  return (
    <ThreadPrimitive.Root
      className="aui-root aui-thread-root @container flex h-full flex-col bg-background"
      style={{
        ["--composer-radius" as string]: "24px",
        ["--composer-padding" as string]: "10px",
      }}
    >
      <ThreadPrimitive.Viewport
        turnAnchor="top"
        className="aui-thread-viewport relative flex flex-1 flex-col overflow-x-auto overflow-y-scroll scroll-smooth px-10 pt-6"
      >
        <AuiIf condition={(s) => s.thread.isEmpty}>
          <ThreadWelcome />
        </AuiIf>

        <ThreadPrimitive.Messages>
          {() => <ThreadMessage />}
        </ThreadPrimitive.Messages>

        <ThreadPrimitive.ViewportFooter className="aui-thread-viewport-footer sticky bottom-0 mt-auto flex w-full flex-col gap-4 overflow-visible rounded-t-(--composer-radius) bg-background pb-4 md:pb-6">
          <ThreadScrollToBottom />
          <FloatingApprovalBanner />
          <Composer />
        </ThreadPrimitive.ViewportFooter>
      </ThreadPrimitive.Viewport>
    </ThreadPrimitive.Root>
  );
};

const ThreadMessage: FC = () => {
  const role = useAuiState((s) => s.message.role);
  const isEditing = useAuiState((s) => s.message.composer.isEditing);
  if (isEditing) return <EditComposer />;
  if (role === "user") return <UserMessage />;
  return <AssistantMessage />;
};

const ThreadScrollToBottom: FC = () => {
  return (
    <ThreadPrimitive.ScrollToBottom asChild>
      <TooltipIconButton
        tooltip="Scroll to bottom"
        variant="outline"
        className="aui-thread-scroll-to-bottom absolute -top-12 z-10 self-center rounded-full p-4 disabled:invisible dark:border-border dark:bg-background dark:hover:bg-accent"
      >
        <ArrowDownIcon />
      </TooltipIconButton>
    </ThreadPrimitive.ScrollToBottom>
  );
};

const ThreadWelcome: FC = () => {
  return (
    <div className="aui-thread-welcome-root mx-auto my-auto flex w-full max-w-(--thread-max-width) grow flex-col">
      <div className="aui-thread-welcome-center flex w-full grow flex-col items-center justify-center">
        <div className="aui-thread-welcome-message flex size-full flex-col items-center justify-center gap-5 px-4">
          <p className="aui-thread-welcome-message-inner fade-in slide-in-from-bottom-1 animate-in fill-mode-both text-[15px] font-medium text-text-secondary duration-200">
            你的专属 AI 团队已就绪
          </p>
          <h1 className="aui-thread-welcome-message-inner fade-in slide-in-from-bottom-1 animate-in fill-mode-both text-[26px] font-bold tracking-[-0.5px] text-foreground delay-75 duration-200">
            今天需要我帮你做些什么？
          </h1>
        </div>
      </div>
      <ThreadSuggestions />
    </div>
  );
};

const ThreadSuggestions: FC = () => {
  return (
    <div className="aui-thread-welcome-suggestions grid w-full @md:grid-cols-2 gap-2 pb-4">
      <ThreadPrimitive.Suggestions>
        {() => <ThreadSuggestionItem />}
      </ThreadPrimitive.Suggestions>
    </div>
  );
};

const ThreadSuggestionItem: FC = () => {
  return (
    <div className="aui-thread-welcome-suggestion-display fade-in slide-in-from-bottom-2 @md:nth-[n+3]:block nth-[n+3]:hidden animate-in fill-mode-both duration-200">
      <SuggestionPrimitive.Trigger send asChild>
        <Button
          variant="ghost"
          className="aui-thread-welcome-suggestion h-auto w-full @md:flex-col flex-wrap items-start justify-start gap-1 rounded-3xl border bg-background px-4 py-3 text-left text-sm transition-colors hover:bg-muted"
        >
          <SuggestionPrimitive.Title className="aui-thread-welcome-suggestion-text-1 font-medium" />
          <SuggestionPrimitive.Description className="aui-thread-welcome-suggestion-text-2 text-muted-foreground empty:hidden" />
        </Button>
      </SuggestionPrimitive.Trigger>
    </div>
  );
};

const Composer: FC = () => {
  const isRunning = useAuiState((s) => s.thread.isRunning);
  const composerRuntime = useComposerRuntime();

  /**
   * 发送消息。
   *
   * DLP 扫描统一在 TauriRuntimeProvider.onNew 中处理（Fail-Safe 设计）。
   * Composer 只负责触发发送，不重复做 DLP。
   */
  const handleSend = useCallback(() => {
    const text = composerRuntime.getState().text.trim();
    if (!text) return;
    composerRuntime.send();
  }, [composerRuntime]);

  /** 拦截 form 提交（Enter 键） */
  const handleFormSubmit = useCallback((e: React.FormEvent) => {
    e.preventDefault();
    handleSend();
  }, [handleSend]);

  const shell = (
    <div
      data-slot="composer-shell"
      className={cn(
        "flex w-full flex-col gap-2 rounded-[17px] border bg-card px-[18px] py-[14px] transition-shadow",
        !isRunning && "shadow-[0_2px_12px_var(--shadow-color)] focus-within:shadow-[0_4px_16px_var(--shadow-color)]",
        isRunning && "aurora-inner",
        "focus-within:border-ring/75 focus-within:ring-2 focus-within:ring-ring/20",
        "data-[dragging=true]:border-ring data-[dragging=true]:border-dashed data-[dragging=true]:bg-accent/50",
      )}
    >
      <ComposerAttachments />
      <ComposerPrimitive.Input
        placeholder="输入消息..."
        className="aui-composer-input max-h-40 min-h-10 w-full resize-none bg-transparent px-1.75 py-1 text-sm outline-none placeholder:text-muted-foreground/80"
        rows={1}
        autoFocus
        aria-label="聊天输入"
      />
      <ComposerSendButton onSend={handleSend} />
    </div>
  );

  return (
    <form className="aui-composer-root relative flex w-full flex-col" onSubmit={handleFormSubmit}>
      {isRunning ? (
        <div className="aurora-glow">{shell}</div>
      ) : (
        shell
      )}
    </form>
  );
};

/** Composer 底部工具栏：左侧模型选择器 + 右侧发送/停止按钮 */
const ComposerSendButton: FC<{ onSend: () => void }> = ({ onSend }) => {
  return (
    <div className="aui-composer-action-wrapper relative flex items-center justify-between">
      {/* 左侧：附件 + 模型选择器 */}
      <div className="flex items-center gap-2">
        <ComposerAddAttachment />
        <ModelSelector />
      </div>

      {/* 右侧：发送 / 停止 */}
      <div className="flex items-center gap-2">
        <AuiIf condition={(s) => !s.thread.isRunning}>
          <TooltipIconButton
            tooltip="发送消息"
            side="bottom"
            type="button"
            variant="default"
            size="icon"
            className="aui-composer-send size-8 rounded-[10px] bg-primary text-primary-foreground hover:bg-primary/90"
            aria-label="发送消息"
            onClick={onSend}
          >
            <ArrowUpIcon className="aui-composer-send-icon size-4" />
          </TooltipIconButton>
        </AuiIf>
        <AuiIf condition={(s) => s.thread.isRunning}>
          <ComposerPrimitive.Cancel asChild>
            <TooltipIconButton
              tooltip="停止生成"
              side="bottom"
              type="button"
              variant="default"
              size="icon"
              className="aui-composer-cancel size-8 rounded-[10px] bg-[#E05A2B] text-white hover:bg-[#C94D24]"
              aria-label="停止生成"
            >
              <SquareIcon className="aui-composer-cancel-icon size-3.5 fill-current" />
            </TooltipIconButton>
          </ComposerPrimitive.Cancel>
        </AuiIf>
      </div>
    </div>
  );
};

const MessageError: FC = () => {
  return (
    <MessagePrimitive.Error>
      <ErrorPrimitive.Root className="aui-message-error-root mt-2 flex max-w-[600px] flex-col gap-2 rounded-tl rounded-tr-2xl rounded-br-2xl rounded-bl-2xl border border-[#FCCDC7] bg-[#FFFAF9] px-4 py-3.5">
        {/* 标题行：图标 + 标题 */}
        <div className="flex items-center gap-2.5">
          <div className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-[#FFF0EE]">
            <XCircleIcon className="size-[18px] text-[#CC3314]" />
          </div>
          <span className="text-[13px] font-semibold text-[#B52A14]">模型调用失败</span>
        </div>
        {/* 错误正文 */}
        <ErrorPrimitive.Message className="aui-message-error-message text-[13px] leading-[1.55] text-[#5A5856]" />
        {/* 分割线 */}
        <div className="h-px bg-[#FCCDC7]" />
        {/* 底部提示 */}
        <div className="flex items-center gap-1.5 text-[11px] text-[#9D9C9A]">
          <SettingsIcon className="size-[11px] shrink-0" />
          <span>如需帮助，请前往设置检查模型配置</span>
        </div>
      </ErrorPrimitive.Root>
    </MessagePrimitive.Error>
  );
};

const AssistantMessage: FC = () => {
  const { pendingApprovals, approve, deny } = useApprovalState();
  const isLast = useAuiState((s) => s.message.isLast);
  const toolSteps = useAuiState((s) => extractToolSteps(s.message.metadata?.custom));
  // Hide lightweight indicators when real tool-call parts are rendered by ToolFallback
  const hasToolCallParts = useAuiState((s) =>
    s.message.content.some((part: { type: string }) => part.type === 'tool-call'),
  );

  return (
    <MessagePrimitive.Root
      className="aui-assistant-message-root fade-in slide-in-from-bottom-1 relative w-full animate-in py-3 duration-150"
      data-role="assistant"
    >
      {/* 头像 + 名称标签 */}
      <div className="mb-1.5 flex items-center gap-2.5">
        <div className="flex size-8 shrink-0 items-center justify-center rounded-full bg-[#2D6B45] text-xs font-medium text-white">
          XC
        </div>
        <span className="text-[11px] font-semibold text-[#9D9C9A]">X-Claw</span>
      </div>

      {/* 内容区：左偏移 42px（头像 32px + gap 10px），与头像右侧对齐 */}
      <div className="aui-assistant-message-content wrap-break-word ml-[42px] text-foreground leading-relaxed">
        <MessagePrimitive.Parts
          components={{
            Text: MarkdownText,
            Reasoning,
            ReasoningGroup,
            tools: { Fallback: ToolFallback },
          }}
        />

        {/* 流式过程中的工具执行步骤时间线（仅当无真实 tool-call 渲染时显示） */}
        {toolSteps && toolSteps.length > 0 && !hasToolCallParts && (
          <div className="my-1.5 flex flex-col gap-0.5 border-l-2 border-muted-foreground/15 pl-3">
            {groupToolSteps(toolSteps).map((segment, idx) =>
              segment.type === "group" ? (
                <ContextToolGroup
                  key={`ctx-group-${segment.steps[0].startedAt}-${idx}`}
                  steps={segment.steps}
                />
              ) : (
                <ToolStepIndicator
                  key={`${segment.step.toolName}-${segment.step.startedAt}-${idx}`}
                  toolName={segment.step.toolName}
                  status={segment.step.status}
                  error={segment.step.error}
                />
              ),
            )}
          </div>
        )}

        <MessageError />
      </div>

      {/* 即时工具授权按钮 */}
      {isLast && pendingApprovals.length > 0 && (
        <div className="ml-[42px] mt-3 flex flex-col gap-2">
          {pendingApprovals.map((approval) => (
            <ApprovalCard
              key={approval.request_id}
              approval={approval}
              onApprove={() => approve(approval.request_id)}
              onDeny={() => deny(approval.request_id)}
            />
          ))}
        </div>
      )}

      <div className="aui-assistant-message-footer mt-1 ml-[42px] flex min-h-6 items-center">
        <BranchPicker />
        <AssistantActionBar />
      </div>
    </MessagePrimitive.Root>
  );
};

const AssistantActionBar: FC = () => {
  const { config: watermark } = useWatermark();

  const handleExport = useCallback((content: string) => {
    const watermarkedContent = watermark.enabled && watermark.text
      ? `${content}\n\n---\n\n*${watermark.text}*`
      : content;

    const blob = new Blob([watermarkedContent], { type: 'text/markdown' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `message-${Date.now()}.md`;
    a.click();
    URL.revokeObjectURL(url);
  }, [watermark.enabled, watermark.text]);

  return (
    <ActionBarPrimitive.Root
      hideWhenRunning
      autohide="not-last"
      className="aui-assistant-action-bar-root col-start-3 row-start-2 -ml-1 flex gap-1 text-muted-foreground"
    >
      <ActionBarPrimitive.Copy asChild>
        <TooltipIconButton tooltip="复制">
          <AuiIf condition={(s) => s.message.isCopied}>
            <CheckIcon />
          </AuiIf>
          <AuiIf condition={(s) => !s.message.isCopied}>
            <CopyIcon />
          </AuiIf>
        </TooltipIconButton>
      </ActionBarPrimitive.Copy>
      <ActionBarPrimitive.Reload asChild>
        <TooltipIconButton tooltip="重新生成">
          <RefreshCwIcon />
        </TooltipIconButton>
      </ActionBarPrimitive.Reload>
      <ActionBarMorePrimitive.Root>
        <ActionBarMorePrimitive.Trigger asChild>
          <TooltipIconButton
            tooltip="更多"
            className="data-[state=open]:bg-accent"
          >
            <MoreHorizontalIcon />
          </TooltipIconButton>
        </ActionBarMorePrimitive.Trigger>
        <ActionBarMorePrimitive.Content
          side="bottom"
          align="start"
          className="aui-action-bar-more-content z-50 min-w-32 overflow-hidden rounded-md border bg-popover p-1 text-popover-foreground shadow-md"
        >
          <ActionBarPrimitive.ExportMarkdown onExport={handleExport} asChild>
            <ActionBarMorePrimitive.Item className="aui-action-bar-more-item flex cursor-pointer select-none items-center gap-2 rounded-sm px-2 py-1.5 text-sm outline-none hover:bg-accent hover:text-accent-foreground focus:bg-accent focus:text-accent-foreground">
              <DownloadIcon className="size-4" />
              导出为 Markdown
            </ActionBarMorePrimitive.Item>
          </ActionBarPrimitive.ExportMarkdown>
        </ActionBarMorePrimitive.Content>
      </ActionBarMorePrimitive.Root>
    </ActionBarPrimitive.Root>
  );
};

const UserMessage: FC = () => {
  // 从消息级 metadata 读取 dlpStats（绑定到具体消息，不受 isLast 影响）
  const messageDlpStats = useAuiState(
    (s) => extractDlpStats(s.message.metadata?.custom)
  );
  const routineTriggerCount = useAuiState(
    (s) => extractRoutineTriggerCount(s.message.metadata?.custom)
  );
  const handleOpenJobsPanel = useCallback(() => {
    window.dispatchEvent(new CustomEvent('open-jobs-panel'));
  }, []);

  return (
    <MessagePrimitive.Root
      className="aui-user-message-root fade-in slide-in-from-bottom-1 grid w-full max-w-(--thread-max-width) animate-in auto-rows-auto grid-cols-[minmax(72px,1fr)_auto] content-start gap-y-2 py-3 duration-150 [&:where(>*)]:col-start-2"
      data-role="user"
    >
      <UserMessageAttachments />

      <div className="aui-user-message-content-wrapper relative col-start-2 min-w-0">
        <div className="aui-user-message-content wrap-break-word rounded-2xl bg-muted px-4 py-2.5 text-foreground">
          <MessagePrimitive.Parts />
        </div>
        <div className="aui-user-action-bar-wrapper absolute top-1/2 left-0 -translate-x-full -translate-y-1/2 pr-2">
          <UserActionBar />
        </div>
      </div>

      {/* DLP 已脱敏 badge — 绑定到消息级 dlpStats，不依赖全局 context */}
      {messageDlpStats && (
        <div className="col-start-2 flex justify-end">
          <span
            className="inline-flex items-center gap-1 rounded bg-[#dcfce7] px-1.5 py-0.5 text-[10px] font-medium text-[#16a34a]"
          >
            <ShieldCheck size={10} />
            已脱敏
          </span>
        </div>
      )}

      {routineTriggerCount > 0 && (
        <div className="col-start-2 flex justify-end">
          <button
            type="button"
            onClick={handleOpenJobsPanel}
            className="inline-flex items-center gap-1 rounded-md border border-amber-200 bg-amber-50 px-2 py-1 text-[11px] font-medium text-amber-700 transition-colors hover:bg-amber-100"
          >
            已触发 {routineTriggerCount} 个事件任务，点击查看
          </button>
        </div>
      )}

      <BranchPicker className="aui-user-branch-picker col-span-full col-start-1 row-start-3 -mr-1 justify-end" />
    </MessagePrimitive.Root>
  );
};

const UserActionBar: FC = () => {
  return (
    <ActionBarPrimitive.Root
      hideWhenRunning
      autohide="not-last"
      className="aui-user-action-bar-root flex flex-col items-end"
    >
      <ActionBarPrimitive.Edit asChild>
        <TooltipIconButton tooltip="编辑" className="aui-user-action-edit p-4">
          <PencilIcon />
        </TooltipIconButton>
      </ActionBarPrimitive.Edit>
    </ActionBarPrimitive.Root>
  );
};

const EditComposer: FC = () => {
  return (
    <MessagePrimitive.Root className="aui-edit-composer-wrapper mx-auto flex w-full max-w-(--thread-max-width) flex-col px-2 py-3">
      <ComposerPrimitive.Root className="aui-edit-composer-root ml-auto flex w-full max-w-[85%] flex-col rounded-2xl bg-muted">
        <ComposerPrimitive.Input
          className="aui-edit-composer-input min-h-14 w-full resize-none bg-transparent p-4 text-foreground text-sm outline-none"
          autoFocus
        />
        <div className="aui-edit-composer-footer mx-3 mb-3 flex items-center gap-2 self-end">
          <ComposerPrimitive.Cancel asChild>
            <Button variant="ghost" size="sm">
              取消
            </Button>
          </ComposerPrimitive.Cancel>
          <ComposerPrimitive.Send asChild>
            <Button size="sm">更新</Button>
          </ComposerPrimitive.Send>
        </div>
      </ComposerPrimitive.Root>
    </MessagePrimitive.Root>
  );
};

const BranchPicker: FC<BranchPickerPrimitive.Root.Props> = ({
  className,
  ...rest
}) => {
  return (
    <BranchPickerPrimitive.Root
      hideWhenSingleBranch
      className={cn(
        "aui-branch-picker-root mr-2 -ml-2 inline-flex items-center text-muted-foreground text-xs",
        className,
      )}
      {...rest}
    >
      <BranchPickerPrimitive.Previous asChild>
        <TooltipIconButton tooltip="上一个">
          <ChevronLeftIcon />
        </TooltipIconButton>
      </BranchPickerPrimitive.Previous>
      <span className="aui-branch-picker-state font-medium">
        <BranchPickerPrimitive.Number /> / <BranchPickerPrimitive.Count />
      </span>
      <BranchPickerPrimitive.Next asChild>
        <TooltipIconButton tooltip="下一个">
          <ChevronRightIcon />
        </TooltipIconButton>
      </BranchPickerPrimitive.Next>
    </BranchPickerPrimitive.Root>
  );
};

// ── 浮动审批横幅（Thread 级，不依赖 branch 位置）──────────────────

const FloatingApprovalBanner: FC = () => {
  const { pendingApprovals, approve, deny } = useApprovalState();
  if (pendingApprovals.length === 0) return null;
  return (
    <div className="mx-auto flex w-full max-w-(--thread-max-width) flex-col gap-2 px-2">
      {pendingApprovals.map((approval) => (
        <ApprovalCard
          key={approval.request_id}
          approval={approval}
          onApprove={() => approve(approval.request_id)}
          onDeny={() => deny(approval.request_id)}
        />
      ))}
    </div>
  );
};

// ── 即时工具授权卡片 ──────────────────────────────────────────────

interface ApprovalCardProps {
  approval: PendingApproval;
  onApprove: () => void;
  onDeny: () => void;
}

const ApprovalCard: FC<ApprovalCardProps> = ({ approval, onApprove, onDeny }) => (
  <div className="flex items-start gap-3 rounded-xl border border-amber-200 bg-amber-50 p-3 dark:border-amber-800/40 dark:bg-amber-950/20">
    <ShieldAlert className="mt-0.5 size-4 shrink-0 text-amber-500" />
    <div className="flex flex-1 flex-col gap-2">
      <div>
        <p className="text-xs font-semibold text-foreground">
          工具执行需要授权：<code className="rounded bg-amber-100 px-1 dark:bg-amber-900/40">{approval.tool_name}</code>
        </p>
        {approval.description && (
          <p className="mt-0.5 text-xs text-muted-foreground">{approval.description}</p>
        )}
      </div>
      <div className="flex gap-2">
        <button
          onClick={onApprove}
          className="flex items-center gap-1 rounded-md bg-primary px-3 py-1 text-xs font-medium text-primary-foreground transition-opacity hover:opacity-90"
        >
          <CheckIcon className="size-3" />
          批准
        </button>
        <button
          onClick={onDeny}
          className="flex items-center gap-1 rounded-md border border-border px-3 py-1 text-xs font-medium text-muted-foreground transition-colors hover:bg-destructive/10 hover:text-destructive"
        >
          <XCircleIcon className="size-3" />
          拒绝
        </button>
      </div>
    </div>
  </div>
);
