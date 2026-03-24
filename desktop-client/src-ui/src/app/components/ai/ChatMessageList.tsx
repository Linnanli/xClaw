/**
 * ChatMessageList - 消息列表容器
 *
 * 自动滚动到底部，支持思考状态、错误提示和内联 DLP 警告。
 *
 * 设计规范（client-design.pen → X-Claw DLP Warning）：
 * - 消息间距 gap: 20px (space-y-5)
 * - DLP 警告内联显示在消息流中（节点 T0iIM）
 * - 思考状态使用 AI 头像 + 气泡样式
 */

import { useEffect, useRef } from 'react';
import { ShieldAlert } from 'lucide-react';
import { ScrollArea } from '../ui/scroll-area';
import { ChatMessage } from './ChatMessage';
import { ThinkingProcess } from './ThinkingProcess';
import type { SanitizationStats } from '../../hooks/useDlpScan';
import type { ThinkingStep } from '../../hooks/useAiChatTauri';

interface Message {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  timestamp?: number;
  dlpStats?: SanitizationStats;
}

/** 内联 DLP 警告信息 */
export interface InlineDlpWarning {
  type: 'redacted' | 'blocked';
  title: string;
  description: string;
}

export interface ChatMessageListProps {
  messages: Message[];
  loading?: boolean;
  thinkingMessage?: string | null;
  /** AI 思考步骤列表（配合 ThinkingProcess 组件） */
  thinkingSteps?: ThinkingStep[];
  error?: string | null;
  /** 内联 DLP 警告（显示在消息流末尾） */
  dlpWarning?: InlineDlpWarning | null;
  onEditMessage?: (id: string) => void;
  onDeleteMessage?: (id: string) => void;
  onRegenerate?: () => void;
}

export function ChatMessageList({
  messages,
  loading,
  thinkingMessage,
  thinkingSteps,
  error,
  dlpWarning,
  onEditMessage,
  onDeleteMessage,
  onRegenerate,
}: ChatMessageListProps) {
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages, thinkingMessage, dlpWarning]);

  return (
    <ScrollArea className="min-h-0 flex-1 px-10 py-6">
      <div className="space-y-5">
        {messages.map((msg) => (
          <ChatMessage
            key={msg.id}
            id={msg.id}
            role={msg.role}
            content={msg.content}
            timestamp={msg.timestamp}
            dlpStats={msg.dlpStats}
            onEdit={onEditMessage}
            onDelete={onDeleteMessage}
            onRegenerate={msg.role === 'assistant' ? onRegenerate : undefined}
          />
        ))}

        {/* 思考过程 - 使用 ThinkingProcess 可折叠组件 */}
        {(thinkingSteps && thinkingSteps.length > 0) || thinkingMessage ? (
          <ThinkingProcess
            steps={thinkingSteps || []}
            isActive={!!thinkingMessage}
          />
        ) : null}

        {/* 加载指示器 */}
        {loading && !thinkingMessage && (
          <ChatMessage
            id="loading"
            role="assistant"
            content=""
            isLoading
          />
        )}

        {/* 内联 DLP 警告（设计稿节点 T0iIM） */}
        {dlpWarning && (
          <div
            className="flex items-center gap-2.5 rounded-xl border border-[#F0D060] bg-[#FFF8E6] px-4 py-3"
            role="alert"
            aria-live="polite"
          >
            <ShieldAlert className="size-[18px] shrink-0 text-[#C8960A]" />
            <div className="flex flex-col gap-0.5">
              <span className="text-[13px] font-semibold text-[#8B6914]">
                {dlpWarning.title}
              </span>
              <span className="text-xs text-[#A68520]">
                {dlpWarning.description}
              </span>
            </div>
          </div>
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
