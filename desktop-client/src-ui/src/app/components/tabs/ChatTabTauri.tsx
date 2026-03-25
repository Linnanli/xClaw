/**
 * ChatTabTauri - 聊天主界面
 *
 * 架构（参考 ChatGPT/Claude 行业惯例）：
 *
 * 欢迎页（入口）：只负责收集用户输入，点发送后立即切换到对话页
 * 对话页（主体）：拿到 pendingMessage 后自己负责 thread 创建、DLP、发送、接收
 *
 * 两个视图职责完全分离，切换零延迟。
 */

import { useState, useEffect, useRef, useCallback } from 'react';
import { MessageSquare, TrendingUp, FileText, Zap } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { threadApi } from '../../utils/tauri';
import { useAiChatTauri } from '../../hooks/useAiChatTauri';
import { useModelConfig } from '../../hooks/useModelConfig';
import { TokenManager } from '../../utils/tokenManager';
import { ChatMessageList } from '../ai/ChatMessageList';
import { ChatInput } from '../ai/ChatInput';
import { ChatWelcome } from '../ai/ChatWelcome';
import { CustomModelModal } from '../ai/CustomModelModal';
import type { QuickAction } from '../ai/QuickActions';
import type { ModelOption } from '../ai/ModelSelector';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '../ui/alert-dialog';
import { DlpBlockedDialog } from '../ai/DlpBlockedDialog';
import type { InlineDlpWarning } from '../ai/ChatMessageList';

/* ===== 常量 ===== */

const QUICK_ACTIONS: QuickAction[] = [
  { icon: MessageSquare, label: '智能对话', prompt: '请帮我进行一次智能对话' },
  { icon: TrendingUp, label: '数据分析', prompt: '请帮我分析以下数据' },
  { icon: FileText, label: '文档处理', prompt: '请帮我处理以下文档' },
  { icon: Zap, label: '技能助手', prompt: '请展示可用的技能列表' },
];

/* ===== 组件 ===== */

interface ChatTabTauriProps {
  selectedThreadId?: string | null;
  onThreadSelect?: (threadId: string) => void;
}

export function ChatTabTauri({ selectedThreadId, onThreadSelect }: ChatTabTauriProps) {
  const [loading, setLoading] = useState(false);
  const [customModelOpen, setCustomModelOpen] = useState(false);
  const [deleteDialog, setDeleteDialog] = useState<{
    open: boolean;
    messageId: string | null;
    preview: string;
  }>({ open: false, messageId: null, preview: '' });

  // ── 核心状态：待发送消息，欢迎页写入，对话页消费 ──
  const [pendingMessage, setPendingMessage] = useState<string | null>(null);

  // 视图状态：是否显示对话视图
  // 有 selectedThreadId（历史对话）或有 pendingMessage（新对话）时进入对话视图
  const isConversationView = !!(selectedThreadId || pendingMessage);

  // 入场动画控制
  const [animateIn, setAnimateIn] = useState(false);
  const prevIsConversation = useRef(false);

  useEffect(() => {
    if (isConversationView && !prevIsConversation.current) {
      // 刚从欢迎页切过来，触发入场动画
      requestAnimationFrame(() => setAnimateIn(true));
    } else if (isConversationView) {
      setAnimateIn(true);
    } else {
      setAnimateIn(false);
    }
    prevIsConversation.current = isConversationView;
  }, [isConversationView]);

  // 模型配置
  const modelConfig = useModelConfig();
  const modelOptions: ModelOption[] = modelConfig.models.map((m) => ({
    id: m.model_id,
    name: m.display_name,
    desc: m.description || '',
    source: m.source,
    isDefault: m.is_default,
  }));

  // 初始化
  useEffect(() => {
    TokenManager.getToken().catch((err) => console.error('Failed to load token:', err));
    invoke('sync_dlp_rules_from_admin').catch((err: unknown) =>
      console.warn('DLP rules sync failed:', err),
    );
  }, []);

  const chat = useAiChatTauri({
    threadId: selectedThreadId || '',
    onError: (error) => console.error('Chat error:', error),
    onStatusChange: (status) => console.log('Chat status:', status),
  });

  // 选择历史对话时加载消息（pendingMessage 触发的新 thread 不加载，避免覆盖乐观更新的消息）
  const isNewThreadFromPending = useRef(false);
  useEffect(() => {
    if (selectedThreadId) {
      if (isNewThreadFromPending.current) {
        // 这是 pendingMessage 流程创建的新 thread，跳过加载（消息已在列表里）
        isNewThreadFromPending.current = false;
        return;
      }
      loadMessages(selectedThreadId);
    }
  }, [selectedThreadId]);

  const loadMessages = async (threadId: string) => {
    try {
      setLoading(true);
      const messages = await threadApi.getMessages(threadId);
      chat.setMessages(
        messages.map((m) => ({
          id: m.id,
          role: m.role as 'user' | 'assistant',
          content: m.content,
          timestamp: new Date(m.created_at).getTime(),
        })),
      );
    } catch (err) {
      console.error('Failed to load messages:', err);
    } finally {
      setLoading(false);
    }
  };

  // ── 对话页消费 pendingMessage：创建 thread + 发送 ──
  const pendingConsumed = useRef(false);
  useEffect(() => {
    if (!pendingMessage || pendingConsumed.current) return;
    pendingConsumed.current = true;

    const processPending = async () => {
      const content = pendingMessage;

      // 立即把用户消息加到列表（对话页已经显示了）
      const tempId = `temp-${Date.now()}`;
      chat.setMessages((prev) => [
        ...prev,
        { id: tempId, role: 'user' as const, content, timestamp: Date.now() },
      ]);

      // 创建 thread
      let threadId = selectedThreadId;
      if (!threadId) {
        try {
          const newThread = await threadApi.createThread();
          threadId = newThread.id;
          isNewThreadFromPending.current = true; // 标记：不要在 useEffect 里 loadMessages
          onThreadSelect?.(newThread.id);
        } catch (err) {
          console.error('Failed to create thread:', err);
          chat.setMessages((prev) => prev.filter((m) => m.id !== tempId));
          setPendingMessage(null);
          pendingConsumed.current = false;
          return;
        }
      }

      // DLP + 发送
      await chat.sendMessageAfterOptimistic(threadId, content, tempId);
      setPendingMessage(null);
      pendingConsumed.current = false;
    };

    processPending();
  }, [pendingMessage, selectedThreadId, onThreadSelect, chat]);

  // ── 欢迎页的发送：只设置 pendingMessage，立即切换 ──
  const handleWelcomeSend = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      const content = chat.input.trim();
      if (!content) return;
      chat.setInput('');
      setPendingMessage(content);
    },
    [chat],
  );

  // ── 对话页的发送：直接走完整流程 ──
  const handleConversationSend = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      const content = chat.input.trim();
      if (!content || chat.isLoading) return;

      chat.setInput('');
      const tempId = `temp-${Date.now()}`;
      chat.setMessages((prev) => [
        ...prev,
        { id: tempId, role: 'user' as const, content, timestamp: Date.now() },
      ]);

      let threadId = selectedThreadId;
      if (!threadId) {
        try {
          const newThread = await threadApi.createThread();
          threadId = newThread.id;
          isNewThreadFromPending.current = true;
          onThreadSelect?.(newThread.id);
        } catch (err) {
          console.error('Failed to create thread:', err);
          chat.setMessages((prev) => prev.filter((m) => m.id !== tempId));
          return;
        }
      }

      await chat.sendMessageAfterOptimistic(threadId, content, tempId);
    },
    [chat, selectedThreadId, onThreadSelect],
  );

  const handleDeleteMessage = (messageId: string) => {
    chat.setMessages((prev) => prev.filter((m) => m.id !== messageId));
    setDeleteDialog({ open: false, messageId: null, preview: '' });
  };

  /* ===== DLP 警告 ===== */
  const inlineDlpWarning: InlineDlpWarning | null =
    chat.dlpWarning?.type === 'redacted' && chat.dlpWarning.stats
      ? {
          type: 'redacted',
          title: 'DLP 安全提示：检测到敏感信息已自动脱敏',
          description: `已按企业安全策略脱敏处理 ${chat.dlpWarning.stats.redacted_count} 处敏感信息`,
        }
      : null;

  /* ===== 欢迎页输入框 props ===== */
  const welcomeInputProps = {
    value: chat.input,
    onChange: chat.setInput,
    onSubmit: handleWelcomeSend,
    onStop: chat.stop,
    isLoading: false, // 欢迎页不显示 loading
    models: modelOptions,
    selectedModel: modelConfig.selectedModelId,
    onModelChange: modelConfig.selectModel,
    onCustomModelClick: () => setCustomModelOpen(true),
  };

  /* ===== 对话页输入框 props ===== */
  const conversationInputProps = {
    value: chat.input,
    onChange: chat.setInput,
    onSubmit: handleConversationSend,
    onStop: chat.stop,
    isLoading: chat.isLoading,
    models: modelOptions,
    selectedModel: modelConfig.selectedModelId,
    onModelChange: modelConfig.selectModel,
    onCustomModelClick: () => setCustomModelOpen(true),
  };

  return (
    <div className="relative flex h-full flex-col bg-background overflow-hidden">
      {/* ── 欢迎页：只是入口 ── */}
      <div
        className={`absolute inset-0 z-10 flex flex-col transition-all duration-300 ease-out ${
          isConversationView
            ? 'pointer-events-none scale-[0.98] opacity-0'
            : 'scale-100 opacity-100'
        }`}
      >
        <ChatWelcome
          quickActions={QUICK_ACTIONS}
          onQuickAction={(prompt) => {
            chat.setInput('');
            setPendingMessage(prompt);
          }}
          {...welcomeInputProps}
        />
      </div>

      {/* ── 对话页：所有逻辑在这里执行 ── */}
      {isConversationView && (
        <div
          className={`flex flex-1 flex-col transition-all duration-300 ease-out ${
            animateIn
              ? 'translate-y-0 opacity-100'
              : 'translate-y-3 opacity-0'
          }`}
        >
          {loading ? (
            <div className="flex flex-1 items-center justify-center text-muted-foreground">
              加载中...
            </div>
          ) : (
            <ChatMessageList
              messages={chat.messages}
              loading={chat.isLoading && !chat.thinkingMessage}
              thinkingMessage={chat.thinkingMessage}
              thinkingSteps={chat.thinkingSteps}
              error={chat.error}
              dlpWarning={inlineDlpWarning}
              onDeleteMessage={(id) => {
                const msg = chat.messages.find((m) => m.id === id);
                setDeleteDialog({
                  open: true,
                  messageId: id,
                  preview: msg?.content.slice(0, 50) || '',
                });
              }}
              onRegenerate={chat.reload}
            />
          )}

          {/* 底部输入框 */}
          <div className="border-t border-border px-10 py-4">
            <div className="mx-auto max-w-[720px]">
              <ChatInput {...conversationInputProps} />
            </div>
          </div>
        </div>
      )}

      {/* 删除确认 */}
      <AlertDialog
        open={deleteDialog.open}
        onOpenChange={(open) => {
          if (!open) setDeleteDialog({ open: false, messageId: null, preview: '' });
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>删除消息</AlertDialogTitle>
            <AlertDialogDescription>
              确定要删除这条消息吗？
              {deleteDialog.preview && (
                <span className="mt-2 block rounded bg-muted p-2 text-xs">
                  &quot;{deleteDialog.preview}{deleteDialog.preview.length >= 50 ? '...' : ''}&quot;
                </span>
              )}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>取消</AlertDialogCancel>
            <AlertDialogAction
              className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
              onClick={() => {
                if (deleteDialog.messageId) handleDeleteMessage(deleteDialog.messageId);
              }}
            >
              删除
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      {/* DLP 阻止对话框 */}
      <DlpBlockedDialog
        open={chat.dlpWarning?.type === 'blocked'}
        onClose={chat.clearDlpWarning}
        onEdit={() => {
          chat.clearDlpWarning();
        }}
        blockReason={chat.dlpWarning?.blockReason}
        stats={chat.dlpWarning?.stats}
      />

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
    </div>
  );
}
