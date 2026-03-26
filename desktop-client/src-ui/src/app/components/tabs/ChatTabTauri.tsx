/**
 * ChatTabTauri - 聊天主界面（Agent 直连 LLM 版本）
 *
 * 架构：
 *   ChatTabTauri（容器）
 *     └─ TauriRuntimeProvider（Tauri IPC ↔ 内嵌 Agent ↔ LLM 直连）
 *         └─ Thread（assistant-ui 官方组件）
 *
 * 数据流：
 *   前端 → Tauri IPC → Agent 消息循环 → LLM API → chat-event → 前端
 *   DLP：发送前通过 Tauri IPC scan_user_input 扫描（保留在客户端）
 *   Skills：Agent 根据消息内容自动激活匹配的 skills
 */

import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Thread } from '@components/assistant-ui/thread';
import { TauriRuntimeProvider, useDlpState } from '../../runtime/TauriRuntimeProvider';
import { useModelConfig } from '@hooks/useModelConfig';
import { TokenManager } from '@utils/tokenManager';
import { CustomModelModal } from '../ai/CustomModelModal';
import { DlpBlockedDialog } from '../ai/DlpBlockedDialog';

interface ChatTabTauriProps {
  selectedThreadId?: string | null;
  onThreadSelect?: (threadId: string) => void;
}

export function ChatTabTauri({ selectedThreadId, onThreadSelect }: ChatTabTauriProps) {
  const [customModelOpen, setCustomModelOpen] = useState(false);
  const modelConfig = useModelConfig();

  useEffect(() => {
    TokenManager.getToken().catch((err) => console.error('Failed to load token:', err));
    invoke('sync_dlp_rules_from_admin').catch((err: unknown) =>
      console.warn('DLP rules sync failed:', err),
    );
  }, []);

  if (modelConfig.loading || !modelConfig.selectedModelId) {
    return <div className="flex h-full items-center justify-center text-sm text-muted-foreground">加载中...</div>;
  }

  return (
    <TauriRuntimeProvider
      key={selectedThreadId ?? 'new'}
      threadId={selectedThreadId ?? null}
      onThreadCreated={(tid) => onThreadSelect?.(tid)}
      modelId={modelConfig.selectedModelId}
    >
      <div className="relative flex h-full flex-col bg-background">
        <Thread />
      </div>

      <DlpBlockedDialogBridge />

      <CustomModelModal
        open={customModelOpen}
        onClose={() => setCustomModelOpen(false)}
        customModels={modelConfig.customModels}
        onSave={async (params) => {
          await modelConfig.createModel({
            model_id: params.model_id,
            display_name: params.display_name,
            provider: 'custom',
            api_base_url: params.api_base_url,
            api_key: params.api_key,
          });
        }}
        onUpdate={async (params) => {
          await modelConfig.updateModel({
            model_id: params.original_model_id,
            display_name: params.display_name,
            api_base_url: params.api_base_url,
            api_key: params.api_key,
          });
        }}
        onDelete={modelConfig.deleteModel}
        onTestConnection={modelConfig.testConnection}
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
