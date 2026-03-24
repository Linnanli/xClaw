/**
 * ChatMessageList - 消息列表容器
 *
 * 自动滚动到底部，支持思考状态和错误提示。
 */

import { useEffect, useRef } from 'react';
import { ScrollArea } from '../ui/scroll-area';
import { ChatMessage } from './ChatMessage';

interface Message {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  timestamp?: number;
}

export interface ChatMessageListProps {
  messages: Message[];
  loading?: boolean;
  thinkingMessage?: string | null;
  error?: string | null;
  onEditMessage?: (id: string) => void;
  onDeleteMessage?: (id: string) => void;
  onRegenerate?: () => void;
}

export function ChatMessageList({
  messages,
  loading,
  thinkingMessage,
  error,
  onEditMessage,
  onDeleteMessage,
  onRegenerate,
}: ChatMessageListProps) {
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages, thinkingMessage]);

  return (
    <ScrollArea className="min-h-0 flex-1 px-6 py-4">
      <div className="mx-auto max-w-3xl space-y-4">
        {messages.map((msg) => (
          <ChatMessage
            key={msg.id}
            id={msg.id}
            role={msg.role}
            content={msg.content}
            timestamp={msg.timestamp}
            onEdit={onEditMessage}
            onDelete={onDeleteMessage}
            onRegenerate={msg.role === 'assistant' ? onRegenerate : undefined}
          />
        ))}

        {/* 思考状态 */}
        {thinkingMessage && (
          <div className="flex justify-start">
            <div className="max-w-[70%] rounded-2xl bg-secondary px-4 py-3">
              <div className="flex items-center gap-2 text-sm text-muted-foreground">
                <span className="animate-pulse">💭</span>
                <span>{thinkingMessage}</span>
              </div>
            </div>
          </div>
        )}

        {/* 加载指示器 */}
        {loading && !thinkingMessage && (
          <ChatMessage
            id="loading"
            role="assistant"
            content=""
            isLoading
          />
        )}

        {/* 错误提示 */}
        {error && (
          <div className="flex justify-center">
            <div className="rounded-lg bg-destructive/10 px-4 py-2 text-sm text-destructive">
              {error}
            </div>
          </div>
        )}

        <div ref={bottomRef} />
      </div>
    </ScrollArea>
  );
}
