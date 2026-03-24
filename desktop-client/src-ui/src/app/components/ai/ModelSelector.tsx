/**
 * ModelSelector - 模型选择器下拉菜单
 *
 * Pencil 设计稿规范：
 * - 触发器：cornerRadius:8, fill:#F5F4F1, height:28, padding:[0,10], stroke:#E5E4E1
 * - Sparkles 图标：fill:#3D8A5A, 13x13
 * - 标签：fontSize:12, fontWeight:600
 * - 下拉面板：cornerRadius:12, width:240, shadow blur:24 color:#00000018 offset:y=8
 * - 选中项：cornerRadius:8, fill:#F0F9F4
 * - 选项文字：fontSize:13, fontWeight:500
 * - 分组标题：fontSize:11, fontWeight:600, fill:#9D9C9A, letterSpacing:0.5
 */

import { useState, useRef, useEffect } from 'react';
import { Sparkles, ChevronDown, Check } from 'lucide-react';
import { cn } from '../ui/utils';

export interface ModelOption {
  id: string;
  name: string;
  desc: string;
  group?: string;
}

export interface ModelSelectorProps {
  models: ModelOption[];
  value: string;
  onChange: (model: string) => void;
}

export function ModelSelector({ models, value, onChange }: ModelSelectorProps) {
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
        <span className="text-foreground">{value}</span>
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
          {models.map((model) => {
            const selected = value === model.name;
            return (
              <button
                key={model.id}
                type="button"
                role="option"
                aria-selected={selected}
                onClick={() => {
                  onChange(model.name);
                  setOpen(false);
                }}
                className={cn(
                  'flex w-full items-center gap-2 rounded-lg px-3 py-2 text-left',
                  'mx-1.5 w-[calc(100%-12px)] transition-colors',
                  selected
                    ? 'bg-[var(--theme-option-active-bg)]'
                    : 'hover:bg-accent',
                )}
              >
                <div className="flex-1">
                  <div className="text-[13px] font-medium text-foreground">
                    {model.name}
                  </div>
                  <div className="text-[11px] text-muted-foreground">
                    {model.desc}
                  </div>
                </div>
                {selected && (
                  <Check className="size-4 shrink-0 text-primary" />
                )}
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
