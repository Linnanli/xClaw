/**
 * ChatTabTauri - 聊天主界面（Data Stream 版本）
 *
 * 架构：
 *   ChatTabTauri（容器）
 *     └─ ChatRuntimeProvider（Data Stream ↔ Admin Backend SSE）
 *         └─ Thread（assistant-ui 官方组件）
 *
 * 数据流：
 *   在线：前端 → Admin Backend /api/chat/completions → LLM API → SSE 响应
 *   离线：前端 → Ollama localhost:11434 → 本地模型 → SSE 响应
 *   DLP：发送前通过 Tauri IPC scan_user_input 扫描（保留在客户端）
 */

import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Thread } from '@components/assistant-ui/thread';
import { ChatRuntimeProvider, useDlpState } from '../../runtime/ChatRuntimeProvider';
import { useModelConfig } from '@hooks/useModelConfig';
import { TokenManager } from '@utils/tokenManager';
import { CustomModelModal } from '../ai/CustomModelModal';
import { DlpBlockedDialog } from '../ai/DlpBlockedDialog';

interface ChatTabTauriProps {
  selectedThreadId?: string | null;
  onThreadSelect?: (threadId: string) => void;
}

export function ChatTabTauri({ selectedThreadId, onThreadSelect: _onThreadSelect }: ChatTabTauriProps) {
  const [customModelOpen, setCustomModelOpen] = useState(false);
  const modelConfig = useModelConfig();

  // 初始化
  useEffect(() => {
    TokenManager.getToken().catch((err) => console.error('Failed to load token:', err));
    invoke('sync_dlp_rules_from_admin').catch((err: unknown) =>
      console.warn('DLP rules sync failed:', err),
    );
  }, []);

  // 根据模型 source 决定 API URL
  const selectedModel = modelConfig.models.find((m) => m.model_id === modelConfig.selectedModelId);
  const isOllama = selectedModel?.provider === 'ollama';
  const apiUrl = isOllama
    ? 'http://localhost:11434/v1/chat/completions'
    : 'http://localhost:3000/api/chat/completions';

  // 模型列表未加载完成时不渲染，避免 modelId 为空导致后端报错
  if (modelConfig.loading || !modelConfig.selectedModelId) {
    return <div className="flex h-full items-center justify-center text-sm text-muted-foreground">加载中...</div>;
  }

  return (
    <ChatRuntimeProvider
      key={selectedThreadId ?? 'new'}
      apiUrl={apiUrl}
      modelId={modelConfig.selectedModelId}
    >
      <div className="relative flex h-full flex-col bg-background">
        <Thread />
      </div>

      {/* DLP 阻止弹窗 */}
      <DlpBlockedDialogBridge />

      {/* 自定义模型弹窗 */}
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
    </ChatRuntimeProvider>
  );
}

/** 桥接 DlpContext → DlpBlockedDialog */
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
