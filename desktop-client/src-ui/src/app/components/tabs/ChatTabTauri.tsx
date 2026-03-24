/**
 * ChatTabTauri - 聊天主界面
 *
 * 使用 shadcn/ui AI 组件重构。
 * 设计稿：无对话时显示欢迎页（标题 + 快捷操作 + 输入框），
 * 有对话时显示消息列表 + 底部输入框。
 *
 * 架构：ChatTabTauri (UI) → useAiChatTauri (状态) → Tauri IPC
 * 状态层 useAiChatTauri 保持不变，仅替换 UI 层。
 */

import { useState, useEffect } from 'react';
import { MessageSquare, TrendingUp, FileText, Zap } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { threadApi } from '../../utils/tauri';
import { useAiChatTauri } from '../../hooks/useAiChatTauri';
import { TokenManager } from '../../utils/tokenManager';
import { ChatMessageList } from '../ai/ChatMessageList';
import { ChatInput } from '../ai/ChatInput';
import { ChatWelcome } from '../ai/ChatWelcome';
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

const AVAILABLE_MODELS: ModelOption[] = [
  { id: 'gpt-4o', name: 'GPT-4o', desc: '最强大的多模态模型' },
  { id: 'gpt-4o-mini', name: 'GPT-4o Mini', desc: '快速且经济' },
  { id: 'claude-3.5', name: 'Claude 3.5', desc: 'Anthropic 旗舰模型' },
  { id: 'deepseek-v3', name: 'DeepSeek V3', desc: '高性价比推理模型' },
];

/* ===== 组件 ===== */

interface ChatTabTauriProps {
  selectedThreadId?: string | null;
  onThreadSelect?: (threadId: string) => void;
}

export function ChatTabTauri({ selectedThreadId, onThreadSelect }: ChatTabTauriProps) {
  const [loading, setLoading] = useState(false);
  const [selectedModel, setSelectedModel] = useState('GPT-4o');
  const [deleteDialog, setDeleteDialog] = useState<{
    open: boolean;
    messageId: string | null;
    preview: string;
  }>({ open: false, messageId: null, preview: '' });

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

  // 选择对话时加载消息
  useEffect(() => {
    if (selectedThreadId) {
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

  const handleSend = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!chat.input.trim() || chat.isLoading) return;

    if (!selectedThreadId) {
      try {
        const newThread = await threadApi.createThread();
        onThreadSelect?.(newThread.id);
      } catch (err) {
        console.error('Failed to create thread:', err);
        return;
      }
    }

    chat.handleSubmit(e);
  };

  const handleDeleteMessage = (messageId: string) => {
    chat.setMessages((prev) => prev.filter((m) => m.id !== messageId));
    setDeleteDialog({ open: false, messageId: null, preview: '' });
  };

  const hasMessages = chat.messages.length > 0;

  /* ===== 内联 DLP 警告（显示在消息流中） ===== */
  const inlineDlpWarning: InlineDlpWarning | null =
    chat.dlpWarning?.type === 'redacted' && chat.dlpWarning.stats
      ? {
          type: 'redacted',
          title: 'DLP 安全提示：检测到敏感信息已自动脱敏',
          description: `已按企业安全策略脱敏处理 ${chat.dlpWarning.stats.redacted_count} 处敏感信息`,
        }
      : null;

  /* ===== 共享的输入框 props ===== */
  const inputProps = {
    value: chat.input,
    onChange: chat.setInput,
    onSubmit: handleSend,
    isLoading: chat.isLoading,
    models: AVAILABLE_MODELS,
    selectedModel,
    onModelChange: setSelectedModel,
  };

  return (
    <div className="flex h-full flex-col bg-background">
      {hasMessages ? (
        <>
          {loading ? (
            <div className="flex flex-1 items-center justify-center text-muted-foreground">
              加载中...
            </div>
          ) : (
            <ChatMessageList
              messages={chat.messages}
              loading={chat.isLoading && !chat.thinkingMessage}
              thinkingMessage={chat.thinkingMessage}
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
          <div className="border-t border-border px-6 py-4">
            <div className="mx-auto max-w-3xl">
              <ChatInput {...inputProps} />
            </div>
          </div>
        </>
      ) : (
        <ChatWelcome
          quickActions={QUICK_ACTIONS}
          onQuickAction={(prompt) => chat.setInput(prompt)}
          {...inputProps}
        />
      )}

      {/* 删除确认 - 使用 shadcn/ui AlertDialog */}
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
                  "{deleteDialog.preview}{deleteDialog.preview.length >= 50 ? '...' : ''}"
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
          // 输入框保留原始内容，用户可以编辑后重新发送
        }}
        blockReason={chat.dlpWarning?.blockReason}
        stats={chat.dlpWarning?.stats}
      />
    </div>
  );
}
