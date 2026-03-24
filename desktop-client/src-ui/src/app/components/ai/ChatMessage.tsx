/**
 * ChatMessage - AI 聊天消息气泡
 *
 * shadcn/ui AI 风格，支持 Markdown 渲染、消息操作（复制/编辑/删除）。
 * 样式来源：Pencil 设计稿 client-design.pen
 *
 * 用户消息：bg-primary (#3D8A5A) 右对齐
 * 助手消息：bg-secondary (#F5F4F1) 左对齐
 */

import { useState, useCallback } from 'react';
import { Copy, Check, Pencil, Trash2, RotateCcw } from 'lucide-react';
import ReactMarkdown from 'react-markdown';
import { cn } from '../ui/utils';
import { Button } from '../ui/button';

export interface ChatMessageProps {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  timestamp?: number;
  isLoading?: boolean;
  onEdit?: (id: string) => void;
  onDelete?: (id: string) => void;
  onRegenerate?: () => void;
}

export function ChatMessage({
  id,
  role,
  content,
  isLoading,
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
    <div className={cn('group flex', isUser ? 'justify-end' : 'justify-start')}>
      <div className="flex max-w-[70%] flex-col gap-1">
        {/* 消息气泡 */}
        <div
          className={cn(
            'rounded-2xl px-4 py-3 text-sm leading-relaxed',
            isUser
              ? 'bg-primary text-primary-foreground'
              : 'bg-secondary text-foreground',
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
