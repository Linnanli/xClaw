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
import { TokenManager } from '@utils/tokenManager';
import { DlpBlockedDialog } from '../ai/DlpBlockedDialog';

interface ChatTabTauriExperimentalProps {
  selectedThreadId?: string | null;
  /** 当前模型 id（由上层状态管理；新 Runtime 暂不支持运行时创建线程回调） */
  selectedModelId?: string | null;
}

export function ChatTabTauriExperimental({
  selectedThreadId,
  selectedModelId,
}: ChatTabTauriExperimentalProps) {
  useEffect(() => {
    TokenManager.getToken().catch((err) => console.error('Failed to load token:', err));
    invoke('sync_dlp_rules_from_admin').catch((err: unknown) =>
      console.warn('DLP rules sync failed:', err),
    );
  }, []);

  return (
    <ChatRuntimeProvider
      threadId={selectedThreadId ?? null}
      modelId={selectedModelId ?? null}
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
