/**
 * ChatInput - 聊天输入框
 *
 * Pencil 设计稿规范：
 * - 容器：cornerRadius:17, fill:#FFFFFF, padding:[14,18], stroke:#E5E4E1 1px
 * - 宽度：720px
 * - 发送按钮：cornerRadius:10, fill:#3D8A5A, 32x32
 * - 附件图标：fill:#9D9C9A, 18x18
 *
 * AI 运行时 (isLoading):
 * - 外层 AuroraGlow 极光边框动画 (cornerRadius:19, padding:2, angular gradient)
 * - 发送按钮变为红色暂停按钮 (fill:#E05A2B, icon:square)
 */

import { useEffect, useRef } from 'react';
import { Paperclip, Image, ArrowUp, Square } from 'lucide-react';
import { cn } from '../ui/utils';
import { ModelSelector, type ModelOption } from './ModelSelector';

export interface ChatInputProps {
  value: string;
  onChange: (value: string) => void;
  onSubmit: (e: React.FormEvent) => void;
  onStop?: () => void;
  isLoading?: boolean;
  models: ModelOption[];
  selectedModel: string;
  onModelChange: (model: string) => void;
  onCustomModelClick?: () => void;
  className?: string;
}

export function ChatInput({
  value,
  onChange,
  onSubmit,
  onStop,
  isLoading,
  models,
  selectedModel,
  onModelChange,
  onCustomModelClick,
  className,
}: ChatInputProps) {
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // 自动调整高度
  useEffect(() => {
    const el = textareaRef.current;
    if (el) {
      el.style.height = 'auto';
      el.style.height = Math.min(el.scrollHeight, 160) + 'px';
    }
  }, [value]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      onSubmit(e);
    }
  };

  /* 输入框主体 */
  const inputBox = (
    <div
      className={cn(
        'rounded-[17px] border border-border bg-card',
        'px-[18px] py-[14px]',
        'transition-shadow',
        !isLoading && 'shadow-[0_2px_12px_var(--shadow-color)]',
        !isLoading && 'focus-within:shadow-[0_4px_16px_var(--shadow-color)]',
        isLoading && 'aurora-inner',
      )}
    >
      {/* 输入区域 */}
      <textarea
        ref={textareaRef}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={handleKeyDown}
        placeholder="输入消息..."
        disabled={isLoading}
        rows={1}
        className={cn(
          'w-full resize-none bg-transparent text-sm text-foreground',
          'placeholder:text-muted-foreground',
          'focus:outline-none disabled:opacity-50',
        )}
        aria-label="聊天输入"
      />

      {/* 底部操作栏 */}
      <div className="mt-3 flex items-center justify-between">
        <ModelSelector
          models={models}
          value={selectedModel}
          onChange={onModelChange}
          onCustomModelClick={onCustomModelClick}
        />

        <div className="flex items-center gap-2">
          <button
            type="button"
            className="text-muted-foreground transition-colors hover:text-foreground"
            aria-label="添加附件"
          >
            <Paperclip className="size-[18px]" />
          </button>
          <button
            type="button"
            className="text-muted-foreground transition-colors hover:text-foreground"
            aria-label="添加图片"
          >
            <Image className="size-[18px]" />
          </button>

          {isLoading ? (
            <button
              type="button"
              onClick={onStop}
              className={cn(
                'flex size-8 items-center justify-center rounded-[10px]',
                'bg-[#E05A2B] text-white transition-colors hover:bg-[#C94D24]',
              )}
              aria-label="停止生成"
            >
              <Square className="size-3.5" fill="currentColor" />
            </button>
          ) : (
            <button
              type="submit"
              disabled={!value.trim()}
              className={cn(
                'flex size-8 items-center justify-center rounded-[10px]',
                'bg-primary text-primary-foreground transition-colors',
                'hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed',
              )}
              aria-label="发送消息"
            >
              <ArrowUp className="size-4" />
            </button>
          )}
        </div>
      </div>
    </div>
  );

  return (
    <form onSubmit={onSubmit} className={className}>
      {isLoading ? (
        <div className="aurora-glow">
          {inputBox}
        </div>
      ) : (
        inputBox
      )}
    </form>
  );
}
