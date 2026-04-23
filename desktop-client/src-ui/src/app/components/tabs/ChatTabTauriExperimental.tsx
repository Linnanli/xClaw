/**
 * ChatTabTauriExperimental — Phase 1 AI-SDK 新 Runtime 体验入口
 *
 * 与 `ChatTabTauri.tsx` 并存的实验性实现，用于真机冒烟 Step H 的
 * 新 Runtime 通路（TauriChatTransport + useChatRuntime + ApprovalToolUI）。
 *
 * 零破坏原则：
 * - 不修改既有 `ChatTabTauri.tsx`、`TauriRuntimeProvider.tsx`、`thread.tsx`
 * - Phase 1.2 已接入 `ModelProvider` / `ApprovalProvider`，`Thread` 内的
 *   `useApprovalState()` 能读取新 Runtime 自己的 approval 队列
 * - Approval 也可通过 `ApprovalToolUI` 按 `toolCallId` 挂到正确 branch 渲染
 *
 * 真机验收步骤见 `e2e/tests/ai-sdk-migration.spec.js`。
 *
 * 等 Step H 冒烟验收通过后，再用此组件替换 `ChatTabTauri`（作为 Phase 2 前置）。
 */

import { useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useComposerRuntime } from '@assistant-ui/react';
import { Thread } from '@components/assistant-ui/thread';
import { useDlpState } from '../../runtime/ChatRuntimeProvider';
import { ThreadHistoryLoader } from '../../runtime/ThreadHistoryLoader';
import { ModelProvider } from '../../runtime/contexts/ModelProvider';
import { useModelContext } from '../../contexts/ModelContext';
import { useEngineReady } from '../../hooks/useEngineReady';
import { TokenManager } from '@utils/tokenManager';
import { threadApi } from '../../utils/tauri';
import { tracing } from '../../utils/tracing';
import type { ChatCommand } from '../../types/chatCommand';
import { DlpBlockedDialog } from '../ai/DlpBlockedDialog';

interface ChatTabTauriExperimentalProps {
  selectedThreadId?: string | null;
  /** 可选：初始模型 id；若未提供由 ModelProvider 自行解析默认模型 */
  selectedModelId?: string | null;
  /** 模型切换回调（上层可据此持久化到应用配置） */
  onModelChange?: (modelId: string) => void;
  /** “自定义模型”按钮点击回调（上层弹窗） */
  onOpenCustomModelModal?: () => void;
  /**
   * 新 thread 被自动创建时的回调（Phase 1.3.e 红线 #7）。
   *
   * 当 `selectedThreadId === null` 且引擎已就绪时，runtime 层会自动调用
   * `threadApi.createThread()` 拿到真实 thread id 并回传给上层，上层应
   * 据此更新 `selectedThreadId`，触发 runtime 以真实 id 重新挂载。
   *
   * 未提供时：`selectedThreadId` 为 null 会渲染 "等待创建 thread" 占位，
   * 不会自动创建（保留上层对 thread 生命周期的完全控制）。
   */
  onThreadCreated?: (threadId: string) => void;
  /**
   * 外部指令入口（Phase 1.4：对齐旧 ChatTabTauri 的 outboundCommand 通道）。
   *
   * 当 `pendingCommand.kind === 'send_text'` 时，runtime 内部会通过
   * `useComposerRuntime` 把文本写入输入框并触发 `send()`，复用 composer
   * 的 DLP 拦截链路。处理完成后调用 `onOutboundCommandHandled(id)` 释放队列。
   */
  outboundCommand?: ChatCommand | null;
  onOutboundCommandHandled?: (commandId: string) => void;
}

export function ChatTabTauriExperimental({
  selectedThreadId,
  selectedModelId,
  onModelChange,
  onOpenCustomModelModal,
  onThreadCreated,
  outboundCommand,
  onOutboundCommandHandled,
}: ChatTabTauriExperimentalProps) {
  useEffect(() => {
    TokenManager.getToken().catch((err) => console.error('Failed to load token:', err));
    invoke('sync_dlp_rules_from_admin').catch((err: unknown) =>
      console.warn('DLP rules sync failed:', err),
    );
  }, []);

  return (
    <ModelProvider
      initialModelId={selectedModelId ?? undefined}
      onModelChange={onModelChange}
      onOpenCustomModelModal={onOpenCustomModelModal}
    >
      <ChatRuntimeBridge
        threadId={selectedThreadId ?? null}
        onThreadCreated={onThreadCreated}
        outboundCommand={outboundCommand ?? null}
        onOutboundCommandHandled={onOutboundCommandHandled}
      />
    </ModelProvider>
  );
}

/**
 * 从 ModelContext 读出当前选中模型的 id / api_base_url / api_key，
 * 透传给 ChatRuntimeProvider 的 transport。
 *
 * 引擎未就绪时渲染启动占位，避免 transport 在 IPC 通道没准备好时就发送请求。
 * `useEngineReady` 由外层 `MainApp` 的 `EngineReadyProvider` 提供，
 * 通过 `chat-stream` 的 `connection_status` 事件翻转 `ready` 标志。
 */
function ChatRuntimeBridge({
  threadId,
  onThreadCreated,
  outboundCommand,
  onOutboundCommandHandled,
}: {
  threadId: string | null;
  onThreadCreated?: (threadId: string) => void;
  outboundCommand?: ChatCommand | null;
  onOutboundCommandHandled?: (commandId: string) => void;
}) {
  const { selectedModelId, models } = useModelContext();
  const { ready } = useEngineReady();
  const selected = models.find((m) => m.model_id === selectedModelId) ?? null;

  // Phase 1.3.e Bootstrap gate:
  // threadId === null 时 **不挂** useChatRuntime，避免 assistant-ui 内部
  // 生成临时 id 进到后端，产生孤儿 thread。由此处统一调 threadApi.createThread，
  // 拿到真实 id 后回调上层切换 selectedThreadId。
  const bootstrappingRef = useRef(false);
  useEffect(() => {
    if (threadId !== null) {
      bootstrappingRef.current = false;
      return;
    }
    if (!ready || !onThreadCreated) return;
    if (bootstrappingRef.current) return;
    bootstrappingRef.current = true;
    threadApi
      .createThread()
      .then((t) => {
        onThreadCreated(t.id);
      })
      .catch((err: unknown) => {
        tracing.error('[ChatTabTauriExperimental] bootstrap thread failed', { error: err });
        bootstrappingRef.current = false; // 允许后续重试
      });
  }, [threadId, ready, onThreadCreated]);

  if (!ready) {
    return (
      <div
        data-testid="chat-runtime-boot-placeholder"
        className="flex h-full flex-col items-center justify-center bg-background text-sm text-muted-foreground"
      >
        <div className="flex items-center gap-2">
          <span className="h-2 w-2 animate-pulse rounded-full bg-amber-500" aria-hidden />
          引擎启动中…
        </div>
      </div>
    );
  }

  if (threadId === null) {
    return (
      <div
        data-testid="chat-runtime-bootstrap-placeholder"
        className="flex h-full flex-col items-center justify-center bg-background text-sm text-muted-foreground"
      >
        <div className="flex items-center gap-2">
          <span className="h-2 w-2 animate-pulse rounded-full bg-emerald-500" aria-hidden />
          {onThreadCreated ? '正在创建新对话…' : '请从侧边栏选择或新建一个对话'}
        </div>
      </div>
    );
  }

  return (
    <ThreadHistoryLoader
      threadId={threadId}
      modelId={selectedModelId || null}
      apiBaseUrl={selected?.api_base_url ?? null}
      apiKey={selected?.api_key ?? null}
    >
      <div className="relative flex h-full flex-col bg-background">
        <Thread />
      </div>
      <DlpBlockedDialogBridge />
      <OutboundCommandBridge
        command={outboundCommand ?? null}
        onConsume={onOutboundCommandHandled}
      />
    </ThreadHistoryLoader>
  );
}

/**
 * OutboundCommandBridge — 外部指令注入到 composer 输入框并触发发送。
 *
 * 必须挂在 `AssistantRuntimeProvider` 内部（即 `ThreadHistoryLoader` →
 * `ChatRuntimeProvider` 的 children 中），否则 `useComposerRuntime()` 会
 * 拿不到 runtime 报错。
 *
 * 用 `lastConsumedIdRef` 防止 strict-mode 双挂载或同一指令被重复发送。
 */
function OutboundCommandBridge({
  command,
  onConsume,
}: {
  command: ChatCommand | null;
  onConsume?: (commandId: string) => void;
}) {
  const composerRuntime = useComposerRuntime();
  const lastConsumedIdRef = useRef<string | null>(null);

  useEffect(() => {
    if (!command) return;
    if (lastConsumedIdRef.current === command.id) return;
    if (command.kind !== 'send_text') return;
    lastConsumedIdRef.current = command.id;
    try {
      composerRuntime.setText(command.text);
      composerRuntime.send();
    } catch (err) {
      tracing.error('[OutboundCommandBridge] inject command failed', { error: err });
    } finally {
      onConsume?.(command.id);
    }
  }, [command, composerRuntime, onConsume]);

  return null;
}

function DlpBlockedDialogBridge() {
  const dlp = useDlpState();
  return (
    <DlpBlockedDialog
      open={dlp.blocked}
      onClose={dlp.clearBlock}
      onEdit={dlp.clearBlock}
      blockReason={dlp.blockReason ?? undefined}
    />
  );
}
