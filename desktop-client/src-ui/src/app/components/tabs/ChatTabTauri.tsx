/**
 * ChatTabTauri - 聊天主界面
 *
 * 设计稿：无对话时显示欢迎页（标题 + 快捷操作 + 输入框），
 * 有对话时显示消息列表 + 底部输入框。
 * 输入框：圆角 16px 卡片，底部模型选择器 + 附件/发送按钮。
 */

import { useState, useEffect, useRef } from 'react';
import {
  MessageSquare,
  TrendingUp,
  FileText,
  Zap,
  Paperclip,
  Image,
  ArrowUp,
  Sparkles,
  ChevronDown,
} from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { threadApi } from '../../utils/tauri';
import { useAiChatTauri } from '../../hooks/useAiChatTauri';
import { TokenManager } from '../../utils/tokenManager';
import { MessageActions } from '../common/MessageActions';
import { MessageEditor } from '../common/MessageEditor';
import { DeleteConfirmDialog } from '../common/DeleteConfirmDialog';
import { ScrollArea } from '../ui/scroll-area';
import { Button } from '../ui/button';
import { cn } from '../ui/utils';

const QUICK_ACTIONS = [
  { icon: MessageSquare, label: '智能对话', prompt: '请帮我进行一次智能对话' },
  { icon: TrendingUp, label: '数据分析', prompt: '请帮我分析以下数据' },
  { icon: FileText, label: '文档处理', prompt: '请帮我处理以下文档' },
  { icon: Zap, label: '技能助手', prompt: '请展示可用的技能列表' },
];

interface ChatTabTauriProps {
  selectedThreadId?: string | null;
  onThreadSelect?: (threadId: string) => void;
}

const AVAILABLE_MODELS = [
  { id: 'gpt-4o', name: 'GPT-4o', desc: '最强大的多模态模型' },
  { id: 'gpt-4o-mini', name: 'GPT-4o Mini', desc: '快速且经济' },
  { id: 'claude-3.5', name: 'Claude 3.5', desc: 'Anthropic 旗舰模型' },
  { id: 'deepseek-v3', name: 'DeepSeek V3', desc: '高性价比推理模型' },
];

export function ChatTabTauri({ selectedThreadId, onThreadSelect }: ChatTabTauriProps) {
  const [loading, setLoading] = useState(false);
  const [selectedModel, setSelectedModel] = useState('GPT-4o');
  const messagesEndRef = useRef<HTMLDivElement>(null);

  // 消息编辑/删除状态
  const [editingMessageId, setEditingMessageId] = useState<string | null>(null);
  const [deleteConfirmDialog, setDeleteConfirmDialog] = useState<{
    isOpen: boolean;
    messageId: string | null;
    content: string;
  }>({ isOpen: false, messageId: null, content: '' });
  const [operationLoading, setOperationLoading] = useState(false);

  // 初始化
  useEffect(() => {
    TokenManager.getToken().catch((err) => {
      console.error('Failed to load token:', err);
    });

    invoke('sync_dlp_rules_from_admin').catch((err: unknown) => {
      console.warn('DLP rules sync failed:', err);
    });
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

  // 自动滚动到底部
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [chat.messages]);

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

    // 如果没有选中对话，先创建一个
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

  const handleDeleteMessage = async (messageId: string) => {
    setOperationLoading(true);
    try {
      chat.setMessages((prev) => prev.filter((m) => m.id !== messageId));
      setDeleteConfirmDialog({ isOpen: false, messageId: null, content: '' });
    } catch (err) {
      console.error('Failed to delete message:', err);
    } finally {
      setOperationLoading(false);
    }
  };

  const handleSaveMessage = async (messageId: string, newContent: string) => {
    setOperationLoading(true);
    try {
      chat.setMessages((prev) =>
        prev.map((m) => (m.id === messageId ? { ...m, content: newContent } : m)),
      );
      setEditingMessageId(null);
    } catch (err) {
      console.error('Failed to save message:', err);
    } finally {
      setOperationLoading(false);
    }
  };

  const hasMessages = chat.messages.length > 0;

  return (
    <div className="flex h-full flex-col bg-background">
      {hasMessages ? (
        /* ===== 消息列表模式 ===== */
        <>
          <ScrollArea className="flex-1 px-6 py-4">
            <div className="mx-auto max-w-3xl space-y-4">
              {loading ? (
                <div className="py-20 text-center text-muted-foreground">加载中...</div>
              ) : (
                chat.messages.map((message) => (
                  <div
                    key={message.id}
                    className={cn(
                      'flex',
                      message.role === 'user' ? 'justify-end' : 'justify-start',
                    )}
                  >
                    <div
                      className={cn(
                        'max-w-[70%] rounded-2xl px-4 py-3',
                        message.role === 'user'
                          ? 'bg-primary text-primary-foreground'
                          : 'bg-secondary text-foreground',
                      )}
                    >
                      {editingMessageId === message.id ? (
                        <MessageEditor
                          messageId={message.id}
                          initialContent={message.content}
                          onSave={handleSaveMessage}
                          onCancel={() => setEditingMessageId(null)}
                          loading={operationLoading}
                        />
                      ) : (
                        <>
                          <div className="whitespace-pre-wrap text-sm">{message.content}</div>
                          {message.role === 'user' && (
                            <MessageActions
                              messageId={message.id}
                              content={message.content}
                              onEdit={() => setEditingMessageId(message.id)}
                              onDelete={() =>
                                setDeleteConfirmDialog({
                                  isOpen: true,
                                  messageId: message.id,
                                  content: message.content,
                                })
                              }
                              onCopy={(content) => navigator.clipboard.writeText(content)}
                            />
                          )}
                        </>
                      )}
                    </div>
                  </div>
                ))
              )}

              {/* 思考状态 */}
              {chat.thinkingMessage && (
                <div className="flex justify-start">
                  <div className="max-w-[70%] rounded-2xl bg-secondary px-4 py-3 text-muted-foreground">
                    <div className="flex items-center gap-2 text-sm">
                      <span className="animate-pulse">💭</span>
                      <span>{chat.thinkingMessage}</span>
                    </div>
                  </div>
                </div>
              )}

              {/* 错误提示 */}
              {chat.error && (
                <div className="flex justify-center">
                  <div className="rounded-lg bg-destructive/10 px-4 py-2 text-sm text-destructive">
                    {chat.error}
                  </div>
                </div>
              )}

              <div ref={messagesEndRef} />
            </div>
          </ScrollArea>

          {/* 底部输入框 */}
          <div className="border-t border-border px-6 py-4">
            <div className="mx-auto max-w-3xl">
              <ChatInput
                value={chat.input}
                onChange={chat.setInput}
                onSubmit={handleSend}
                isLoading={chat.isLoading}
                selectedModel={selectedModel}
                onModelChange={setSelectedModel}
              />
            </div>
          </div>
        </>
      ) : (
        /* ===== 欢迎页模式 ===== */
        <div className="flex flex-1 flex-col items-center justify-center px-10 py-10">
          <div className="flex flex-col items-center gap-5">
            <p className="text-[15px] font-medium text-text-secondary">
              你的专属 AI 团队已就绪
            </p>
            <h2 className="text-[26px] font-bold tracking-[-0.5px] text-foreground">
              今天需要我帮你做些什么？
            </h2>

            {/* 快捷操作 */}
            <div className="flex items-center gap-2.5">
              {QUICK_ACTIONS.map((action) => (
                <button
                  key={action.label}
                  onClick={() => chat.setInput(action.prompt)}
                  className="flex items-center gap-1.5 rounded-full border border-border bg-background px-4 py-2 text-[13px] font-medium text-foreground transition-colors hover:bg-secondary"
                >
                  <action.icon className="size-3.5 text-primary" />
                  {action.label}
                </button>
              ))}
            </div>

            {/* 输入框 */}
            <div className="w-[720px] max-w-full">
              <ChatInput
                value={chat.input}
                onChange={chat.setInput}
                onSubmit={handleSend}
                isLoading={chat.isLoading}
                selectedModel={selectedModel}
                onModelChange={setSelectedModel}
              />
            </div>
          </div>
        </div>
      )}

      {/* 删除确认对话框 */}
      <DeleteConfirmDialog
        isOpen={deleteConfirmDialog.isOpen}
        title="删除消息"
        message={`确定要删除这条消息吗？\n\n"${deleteConfirmDialog.content.slice(0, 50)}${deleteConfirmDialog.content.length > 50 ? '...' : ''}"`}
        onConfirm={() => {
          if (deleteConfirmDialog.messageId) {
            handleDeleteMessage(deleteConfirmDialog.messageId);
          }
        }}
        onCancel={() => setDeleteConfirmDialog({ isOpen: false, messageId: null, content: '' })}
        loading={operationLoading}
      />
    </div>
  );
}

/* ===== ChatInput 子组件 ===== */

interface ChatInputProps {
  value: string;
  onChange: (value: string) => void;
  onSubmit: (e: React.FormEvent) => void;
  isLoading: boolean;
  selectedModel: string;
  onModelChange?: (model: string) => void;
}

function ChatInput({ value, onChange, onSubmit, isLoading, selectedModel, onModelChange }: ChatInputProps) {
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const [modelOpen, setModelOpen] = useState(false);
  const modelRef = useRef<HTMLDivElement>(null);

  // 自动调整高度
  useEffect(() => {
    const el = textareaRef.current;
    if (el) {
      el.style.height = 'auto';
      el.style.height = Math.min(el.scrollHeight, 160) + 'px';
    }
  }, [value]);

  // 点击外部关闭模型选择器
  useEffect(() => {
    if (!modelOpen) return;
    const handleClickOutside = (e: MouseEvent) => {
      if (modelRef.current && !modelRef.current.contains(e.target as Node)) {
        setModelOpen(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [modelOpen]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      onSubmit(e);
    }
  };

  return (
    <form onSubmit={onSubmit}>
      <div className="rounded-2xl border border-border bg-background p-4 shadow-[0_2px_12px_#1A191808]">
        {/* 输入行 */}
        <div className="flex items-start gap-2">
          <span className="mt-0.5 text-base font-semibold text-muted-foreground">@</span>
          <textarea
            ref={textareaRef}
            value={value}
            onChange={(e) => onChange(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="请输入您的需求，或上传文件，AI 将为您解决问题。"
            disabled={isLoading}
            rows={1}
            className="flex-1 resize-none bg-transparent text-sm text-foreground placeholder:text-muted-foreground focus:outline-none disabled:opacity-50"
          />
        </div>

        {/* 底部操作栏 */}
        <div className="mt-3 flex items-center justify-between">
          {/* 模型选择器 */}
          <div className="relative" ref={modelRef}>
            <button
              type="button"
              onClick={() => setModelOpen(!modelOpen)}
              className="flex h-7 items-center gap-1.5 rounded-lg border border-border bg-secondary px-2.5 text-xs font-semibold transition-colors hover:bg-accent"
            >
              <Sparkles className="size-[13px] text-primary" />
              <span>{selectedModel}</span>
              <ChevronDown className="size-[13px] text-muted-foreground" />
            </button>
            {modelOpen && (
              <div className="absolute bottom-full left-0 z-50 mb-1 w-56 rounded-lg border border-border bg-popover p-1 shadow-lg">
                {AVAILABLE_MODELS.map((model) => (
                  <button
                    key={model.id}
                    type="button"
                    onClick={() => {
                      onModelChange?.(model.name);
                      setModelOpen(false);
                    }}
                    className={cn(
                      'flex w-full flex-col rounded-md px-3 py-2 text-left transition-colors hover:bg-accent',
                      selectedModel === model.name && 'bg-primary/10',
                    )}
                  >
                    <span className="text-xs font-semibold text-foreground">{model.name}</span>
                    <span className="text-[11px] text-muted-foreground">{model.desc}</span>
                  </button>
                ))}
              </div>
            )}
          </div>

          {/* 右侧操作 */}
          <div className="flex items-center gap-2">
            <button
              type="button"
              className="text-muted-foreground transition-colors hover:text-foreground"
              aria-label="附件"
            >
              <Paperclip className="size-[18px]" />
            </button>
            <button
              type="button"
              className="text-muted-foreground transition-colors hover:text-foreground"
              aria-label="图片"
            >
              <Image className="size-[18px]" />
            </button>
            <Button
              type="submit"
              size="sm"
              disabled={isLoading || !value.trim()}
              className="h-auto !px-3 !py-1.5 rounded-[10px] [&>svg]:!size-4"
              aria-label="发送"
            >
              <ArrowUp className="size-4" />
            </Button>
          </div>
        </div>
      </div>
    </form>
  );
}
