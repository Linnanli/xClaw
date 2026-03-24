/**
 * ChatInput - 聊天输入框
 *
 * Pencil 设计稿规范：
 * - 容器：cornerRadius:16, fill:#FFFFFF, padding:16, stroke:#E5E4E1 1px
 * - 阴影：blur:12, color:#1A191808, offset:y=2
 * - @ 符号：fontSize:16, fontWeight:600, fill:#9C9B99
 * - 占位符：fontSize:14, fontWeight:normal, fill:#9C9B99
 * - 宽度：720px
 * - 发送按钮：cornerRadius:10, fill:#3D8A5A, padding:[6,12]
 * - 附件图标：fill:#9C9B99, 18x18
 */

import { useEffect, useRef } from 'react';
import { Paperclip, Image, ArrowUp } from 'lucide-react';
import { cn } from '../ui/utils';
import { Button } from '../ui/button';
import { ModelSelector, type ModelOption } from './ModelSelector';

export interface ChatInputProps {
  value: string;
  onChange: (value: string) => void;
  onSubmit: (e: React.FormEvent) => void;
  isLoading?: boolean;
  models: ModelOption[];
  selectedModel: string;
  onModelChange: (model: string) => void;
  className?: string;
}

export function ChatInput({
  value,
  onChange,
  onSubmit,
  isLoading,
  models,
  selectedModel,
  onModelChange,
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

  return (
    <form onSubmit={onSubmit} className={className}>
      <div
        className={cn(
          'rounded-2xl border border-border bg-card p-4',
          'shadow-[0_2px_12px_var(--shadow-color)]',
          'transition-shadow focus-within:shadow-[0_4px_16px_var(--shadow-color)]',
        )}
      >
        {/* 输入区域 */}
        <div className="flex items-start gap-2">
          <span className="mt-0.5 select-none text-base font-semibold text-muted-foreground">
            @
          </span>
          <textarea
            ref={textareaRef}
            value={value}
            onChange={(e) => onChange(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="请输入您的需求，或上传文件，AI 将为您解决问题。"
            disabled={isLoading}
            rows={1}
            className={cn(
              'flex-1 resize-none bg-transparent text-sm text-foreground',
              'placeholder:text-muted-foreground',
              'focus:outline-none disabled:opacity-50',
            )}
            aria-label="聊天输入"
          />
        </div>

        {/* 底部操作栏 */}
        <div className="mt-3 flex items-center justify-between">
          <ModelSelector
            models={models}
            value={selectedModel}
            onChange={onModelChange}
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
            <Button
              type="submit"
              size="sm"
              disabled={isLoading || !value.trim()}
              className="h-auto rounded-[10px] !px-3 !py-1.5 [&>svg]:!size-4"
              aria-label="发送消息"
            >
              <ArrowUp className="size-4" />
            </Button>
          </div>
        </div>
      </div>
    </form>
  );
}
