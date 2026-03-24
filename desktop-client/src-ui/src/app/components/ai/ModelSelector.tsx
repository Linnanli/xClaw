/**
 * ModelSelector - 模型选择器下拉菜单
 *
 * Pencil 设计稿 WJK0X 规范：
 * - 触发器：cornerRadius:8, fill:#F5F4F1, height:28, padding:[0,10], stroke:#E5E4E1
 * - Sparkles 图标：fill:#3D8A5A, 13x13
 * - 标签：fontSize:12, fontWeight:600
 * - 下拉面板：cornerRadius:12, width:240, shadow blur:24 color:#00000018 offset:y=8
 * - 标题：fontSize:11, fontWeight:600, fill:#9D9C9A, letterSpacing:0.5
 * - 选中项：cornerRadius:8, fill:#F0F9F4, fontWeight:600
 * - 未选中项：fontSize:13, fontWeight:500, fill:#4A4947
 * - 推荐角标：cornerRadius:100, fill:#C8F0D8, fontSize:10
 * - 分隔线：fill:#EEEDE9, height:1
 * - 自定义模型按钮：CirclePlus 图标 + ChevronRight 箭头
 */

import { useState, useRef, useEffect } from 'react';
import { Sparkles, ChevronDown, Check, CirclePlus, ChevronRight } from 'lucide-react';
import { cn } from '../ui/utils';

export interface ModelOption {
  id: string;
  name: string;
  desc: string;
  source?: string;       // 'admin' | 'custom' | 'builtin'
  isDefault?: boolean;
}

export interface ModelSelectorProps {
  models: ModelOption[];
  value: string;
  onChange: (modelId: string) => void;
  onCustomModelClick?: () => void;
}

export function ModelSelector({ models, value, onChange, onCustomModelClick }: ModelSelectorProps) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, [open]);

  const selectedModel = models.find((m) => m.id === value);
  const displayName = selectedModel?.name || value;

  return (
    <div className="relative" ref={ref}>
      {/* 触发器 */}
      <button
        type="button"
        onClick={() => setOpen(!open)}
        className={cn(
          'flex h-7 items-center gap-1.5 rounded-lg border border-border',
          'bg-secondary px-2.5 text-xs font-semibold',
          'transition-colors hover:bg-accent',
        )}
        aria-haspopup="listbox"
        aria-expanded={open}
      >
        <Sparkles className="size-[13px] text-primary" />
        <span className="text-foreground">{displayName}</span>
        <ChevronDown className="size-[13px] text-muted-foreground" />
      </button>

      {/* 下拉面板 */}
      {open && (
        <div
          role="listbox"
          className={cn(
            'absolute bottom-full left-0 z-50 mb-1.5',
            'w-60 rounded-xl border border-border bg-popover py-1.5',
            'shadow-[0_8px_24px_rgba(0,0,0,0.09)]',
          )}
        >
          {/* 标题 */}
          <div className="px-3 py-1.5">
            <span className="text-[11px] font-semibold tracking-[0.5px] text-muted-foreground">
              选择模型
            </span>
          </div>

          {/* 模型选项 */}
          {models.map((model) => {
            const selected = value === model.id;
            return (
              <button
                key={model.id}
                type="button"
                role="option"
                aria-selected={selected}
                onClick={() => {
                  onChange(model.id);
                  setOpen(false);
                }}
                className={cn(
                  'flex w-full items-center justify-between',
                  'mx-1.5 w-[calc(100%-12px)] rounded-lg px-3',
                  'h-[38px] transition-colors',
                  selected
                    ? 'bg-[#F0F9F4]'
                    : 'hover:bg-accent',
                )}
              >
                <div className="flex items-center gap-2">
                  <Sparkles
                    className={cn(
                      'size-[13px]',
                      selected ? 'text-primary' : 'text-muted-foreground',
                    )}
                  />
                  <span
                    className={cn(
                      'text-[13px]',
                      selected
                        ? 'font-semibold text-foreground'
                        : 'font-medium text-[#4A4947]',
                    )}
                  >
                    {model.name}
                  </span>
                  {model.isDefault && (
                    <span className="rounded-full bg-[#C8F0D8] px-1.5 py-0.5 text-[10px] font-medium text-primary">
                      推荐
                    </span>
                  )}
                </div>
                {selected && (
                  <Check className="size-[13px] shrink-0 text-primary" />
                )}
              </button>
            );
          })}

          {/* 分隔线 */}
          <div className="mx-3 my-1.5 h-px bg-[#EEEDE9]" />

          {/* 自定义模型入口 */}
          <button
            type="button"
            onClick={() => {
              setOpen(false);
              onCustomModelClick?.();
            }}
            className={cn(
              'flex w-full items-center justify-between',
              'mx-1.5 w-[calc(100%-12px)] rounded-lg px-3',
              'h-[38px] transition-colors hover:bg-accent',
            )}
          >
            <div className="flex items-center gap-2">
              <CirclePlus className="size-[13px] text-[#6D6C6A]" />
              <span className="text-[13px] font-medium text-[#4A4947]">
                自定义模型
              </span>
            </div>
            <ChevronRight className="size-[13px] text-[#B0AFAC]" />
          </button>
        </div>
      )}
    </div>
  );
}
