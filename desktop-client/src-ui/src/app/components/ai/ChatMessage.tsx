/**
 * ChatMessage - AI 聊天消息气泡
 *
 * shadcn/ui AI 风格，支持 Markdown 渲染、消息操作（复制/编辑/删除）。
 * 样式来源：Pencil 设计稿 client-design.pen → X-Claw DLP Warning (IRka8)
 *
 * 视觉规范（设计稿节点）：
 * - 用户消息 (D93sD)：右对齐，头像 #3D8A5A 32px，气泡 #3D8A5A 圆角 [16,16,4,16]
 * - 助手消息 (axHMH)：左对齐，头像 #2D6B45 32px，气泡 #FFFFFF 边框 #E5E4E1 圆角 [16,16,16,4]
 * - 名称标签：fontSize 11, fontWeight 600, fill #9D9C9A
 * - 气泡 padding: [12,16]，文字 fontSize 14
 * - 气泡区域偏移 42px（头像 32px + gap 10px）
 */

import { useState, useCallback } from 'react';
import { Copy, Check, Pencil, Trash2, RotateCcw } from 'lucide-react';
import ReactMarkdown from 'react-markdown';
import { cn } from '../ui/utils';
import { Button } from '../ui/button';
import { Avatar, AvatarFallback } from '../ui/avatar';
import { DlpMessageBadge } from './DlpMessageBadge';
import type { SanitizationStats } from '../../hooks/useDlpScan';

export interface ChatMessageProps {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  timestamp?: number;
  isLoading?: boolean;
  /** DLP 脱敏统计（仅用户消息） */
  dlpStats?: SanitizationStats;
  /** 用户显示名称，默认 "你" */
  userName?: string;
  /** AI 显示名称，默认 "X-Claw" */
  aiName?: string;
  onEdit?: (id: string) => void;
  onDelete?: (id: string) => void;
  onRegenerate?: () => void;
}

export function ChatMessage({
  id,
  role,
  content,
  isLoading,
  dlpStats,
  userName = '你',
  aiName = 'X-Claw',
  onEdit,
  onDelete,
  onRegenerate,
}: ChatMessageProps) {
  const [copied, setCopied] = useState(false);

  const handleCopy = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(content);
    } catch {
      const ta = document.createElement('textarea');
      ta.value = content;
      document.body.appendChild(ta);
      ta.select();
      document.execCommand('copy');
      document.body.removeChild(ta);
    }
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }, [content]);

  const isUser = role === 'user';

  return (
    <div
      className={cn(
        'group flex flex-col gap-1.5',
        isUser ? 'items-end' : 'items-start',
      )}
    >
      {/* 头像 + 名称标签 */}
      <div
        className={cn(
          'flex items-center gap-2.5',
          isUser ? 'flex-row-reverse' : 'flex-row',
        )}
      >
        <Avatar className="size-8">
          <AvatarFallback
            className={cn(
              'text-xs font-medium text-white',
              isUser ? 'bg-[#3D8A5A]' : 'bg-[#2D6B45]',
            )}
          >
            {isUser ? userName.charAt(0) : 'XC'}
          </AvatarFallback>
        </Avatar>
        <span className="text-[11px] font-semibold text-[#9D9C9A]">
          {isUser ? userName : aiName}
        </span>
      </div>

      {/* 气泡区域（偏移 42px 对齐头像下方） */}
      <div
        className={cn(
          'flex max-w-[70%] flex-col gap-1',
          isUser ? 'pr-[42px]' : 'pl-[42px]',
        )}
      >
        {/* 消息气泡 */}
        <div
          className={cn(
            'px-4 py-3 text-sm leading-relaxed',
            isUser
              ? 'rounded-tl-2xl rounded-tr-2xl rounded-br rounded-bl-2xl bg-[#3D8A5A] text-white'
              : 'rounded-tl-2xl rounded-tr-2xl rounded-br-2xl rounded-bl border border-[#E5E4E1] bg-white text-[#4A4947]',
          )}
        >
          {isLoading ? (
            <span className="flex items-center gap-2">
              <span className="inline-block size-1.5 animate-bounce rounded-full bg-current [animation-delay:0ms]" />
              <span className="inline-block size-1.5 animate-bounce rounded-full bg-current [animation-delay:150ms]" />
              <span className="inline-block size-1.5 animate-bounce rounded-full bg-current [animation-delay:300ms]" />
            </span>
          ) : isUser ? (
            <div className="whitespace-pre-wrap">{content}</div>
          ) : (
            <div className="prose prose-sm max-w-none dark:prose-invert prose-p:my-1 prose-pre:my-2 prose-ul:my-1 prose-ol:my-1">
              <ReactMarkdown>{content}</ReactMarkdown>
            </div>
          )}
        </div>

        {/* DLP 脱敏标记 */}
        {isUser && dlpStats && (
          <div className={cn('flex', isUser ? 'justify-end' : 'justify-start')}>
            <DlpMessageBadge stats={dlpStats} />
          </div>
        )}

        {/* 操作栏 - hover 时显示 */}
        {!isLoading && (
          <div
            className={cn(
              'flex items-center gap-0.5 opacity-0 transition-opacity group-hover:opacity-100',
              isUser ? 'justify-end' : 'justify-start',
            )}
          >
            <ActionButton
              icon={copied ? <Check className="size-3.5" /> : <Copy className="size-3.5" />}
              label="复制"
              onClick={handleCopy}
            />
            {isUser && onEdit && (
              <ActionButton
                icon={<Pencil className="size-3.5" />}
                label="编辑"
                onClick={() => onEdit(id)}
              />
            )}
            {isUser && onDelete && (
              <ActionButton
                icon={<Trash2 className="size-3.5" />}
                label="删除"
                onClick={() => onDelete(id)}
                variant="destructive"
              />
            )}
            {!isUser && onRegenerate && (
              <ActionButton
                icon={<RotateCcw className="size-3.5" />}
                label="重新生成"
                onClick={onRegenerate}
              />
            )}
          </div>
        )}
      </div>
    </div>
  );
}


/* ===== 内部子组件 ===== */

function ActionButton({
  icon,
  label,
  onClick,
  variant,
}: {
  icon: React.ReactNode;
  label: string;
  onClick: () => void;
  variant?: 'destructive';
}) {
  return (
    <Button
      variant="ghost"
      size="icon"
      className={cn(
        'size-7 rounded-md text-muted-foreground',
        variant === 'destructive'
          ? 'hover:bg-destructive/10 hover:text-destructive'
          : 'hover:bg-accent hover:text-foreground',
      )}
      onClick={onClick}
      aria-label={label}
    >
      {icon}
    </Button>
  );
}
