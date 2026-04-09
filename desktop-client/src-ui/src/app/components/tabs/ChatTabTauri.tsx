/**
 * ChatTabTauri - 聊天主界面（Agent 直连 LLM 版本）
 *
 * 架构：
 *   ChatTabTauri（容器）
 *     └─ TauriRuntimeProvider（Tauri IPC ↔ 内嵌 Agent ↔ LLM 直连）
 *         ├─ ModelContext（模型列表 + 选择，供 Composer 内 ModelSelector 消费）
 *         └─ Thread（assistant-ui 官方组件）
 *
 * 模型切换流程：
 *   用户点击 ModelSelector → ModelContext.selectModel → modelIdRef 同步 →
 *   下一次 onNew 调用时自动使用新 modelId（无需 key 重置 runtime）
 */

import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Thread } from '@components/assistant-ui/thread';
import { TauriRuntimeProvider, useDlpState } from '../../runtime/TauriRuntimeProvider';
import { TokenManager } from '@utils/tokenManager';
import { CustomModelModal } from '../ai/CustomModelModal';
import { DlpBlockedDialog } from '../ai/DlpBlockedDialog';
import { useModelConfig } from '@hooks/useModelConfig';
import type { ChatCommand } from '../../types/chatCommand';

interface ChatTabTauriProps {
  selectedThreadId?: string | null;
  onThreadSelect?: (threadId: string) => void;
  outboundCommand?: ChatCommand | null;
  onOutboundCommandHandled?: (commandId: string) => void;
  /** 引擎就绪计数器，变化时重新加载历史消息 */
  engineReadyKey?: number;
}

export function ChatTabTauri({
  selectedThreadId,
  onThreadSelect,
  outboundCommand,
  onOutboundCommandHandled,
  engineReadyKey,
}: ChatTabTauriProps) {
  const [customModelOpen, setCustomModelOpen] = useState(false);
  // 模型选择状态提升到此层，避免 TauriRuntimeProvider 因 key 变化重新挂载时丢失
  const [selectedModelId, setSelectedModelId] = useState<string | undefined>(undefined);
  // useModelConfig 仅用于 CustomModelModal 的 CRUD 操作，
  // 模型列表和选择状态由 TauriRuntimeProvider 内部的 ModelContext 管理
  const { customModels, createModel, updateModel, deleteModel, testConnection } = useModelConfig();

  useEffect(() => {
    TokenManager.getToken().catch((err) => console.error('Failed to load token:', err));
    invoke('sync_dlp_rules_from_admin').catch((err: unknown) =>
      console.warn('DLP rules sync failed:', err),
    );
  }, []);

  return (
    <TauriRuntimeProvider
      threadId={selectedThreadId ?? null}
      initialModelId={selectedModelId}
      onThreadCreated={(tid) => onThreadSelect?.(tid)}
      onModelChange={setSelectedModelId}
      onOpenCustomModelModal={() => setCustomModelOpen(true)}
      outboundCommand={outboundCommand}
      onOutboundCommandHandled={onOutboundCommandHandled}
      engineReadyKey={engineReadyKey}
    >
      <div className="relative flex h-full flex-col bg-background">
        <Thread />
      </div>

      <DlpBlockedDialogBridge />

      <CustomModelModal
        open={customModelOpen}
        onClose={() => setCustomModelOpen(false)}
        customModels={customModels}
        onSave={async (params) => {
          await createModel({
            model_id: params.model_id,
            display_name: params.display_name,
            provider: 'custom',
            api_base_url: params.api_base_url,
            api_key: params.api_key,
          });
        }}
        onUpdate={async (params) => {
          await updateModel({
            model_id: params.original_model_id,
            display_name: params.display_name,
            api_base_url: params.api_base_url,
            api_key: params.api_key,
          });
        }}
        onDelete={deleteModel}
        onTestConnection={testConnection}
      />
    </TauriRuntimeProvider>
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
