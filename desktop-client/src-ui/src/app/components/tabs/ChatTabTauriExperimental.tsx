/**
 * ChatTabTauriExperimental — Phase 1 AI-SDK 新 Runtime 体验入口
 *
 * 与 `ChatTabTauri.tsx` 并存的实验性实现，用于真机冒烟 Step H 的
 * 新 Runtime 通路（TauriChatTransport + useChatRuntime + ApprovalToolUI）。
 *
 * 零破坏原则：
 * - 不修改既有 `ChatTabTauri.tsx`、`TauriRuntimeProvider.tsx`、`thread.tsx`
 * - `Thread` 内部仍会 `useApprovalState()`，但新 Provider 不注入 ApprovalContext
 *   → hook 读默认空数组，`FloatingApprovalBanner` 自动空转
 * - Approval 渲染由 `ApprovalToolUI` 按 `toolCallId` 挂到正确 branch
 *
 * 真机验收步骤见 `e2e/tests/ai-sdk-migration.spec.js`。
 *
 * 等 Step H 冒烟验收通过后，再用此组件替换 `ChatTabTauri`（作为 Phase 2 前置）。
 */

import { useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Thread } from '@components/assistant-ui/thread';
import { ChatRuntimeProvider, useDlpState } from '../../runtime/ChatRuntimeProvider';
import { ModelProvider } from '../../runtime/contexts/ModelProvider';
import { useModelContext } from '../../contexts/ModelContext';
import { TokenManager } from '@utils/tokenManager';
import { DlpBlockedDialog } from '../ai/DlpBlockedDialog';

interface ChatTabTauriExperimentalProps {
  selectedThreadId?: string | null;
  /** 可选：初始模型 id；若未提供由 ModelProvider 自行解析默认模型 */
  selectedModelId?: string | null;
  /** 模型切换回调（上层可据此持久化到应用配置） */
  onModelChange?: (modelId: string) => void;
  /** “自定义模型”按钮点击回调（上层弹窗） */
  onOpenCustomModelModal?: () => void;
}

export function ChatTabTauriExperimental({
  selectedThreadId,
  selectedModelId,
  onModelChange,
  onOpenCustomModelModal,
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
      <ChatRuntimeBridge threadId={selectedThreadId ?? null} />
    </ModelProvider>
  );
}

/**
 * 从 ModelContext 读出当前选中模型的 id / api_base_url / api_key，
 * 透传给 ChatRuntimeProvider 的 transport。
 */
function ChatRuntimeBridge({ threadId }: { threadId: string | null }) {
  const { selectedModelId, models } = useModelContext();
  const selected = models.find((m) => m.model_id === selectedModelId) ?? null;
  return (
    <ChatRuntimeProvider
      threadId={threadId}
      modelId={selectedModelId || null}
      apiBaseUrl={selected?.api_base_url ?? null}
      apiKey={selected?.api_key ?? null}
    >
      <div className="relative flex h-full flex-col bg-background">
        <Thread />
      </div>
      <DlpBlockedDialogBridge />
    </ChatRuntimeProvider>
  );
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
