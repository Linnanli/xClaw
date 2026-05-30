/**
 * ModelSelector - 模型选择器下拉菜单
 *
 * Pencil 设计稿 "X-Claw Chat V2 - Model Select - Theme Toggle" 规范：
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
 *
 * 消费方式：通过 ModelContext 获取数据，不接受 models/value/onChange props，
 * 保持组件职责单一（展示 + 交互），数据来源统一由 context 管理。
 */

import { useState, useRef, useEffect } from 'react';
import { Sparkles, ChevronDown, Check, CirclePlus, ChevronRight } from 'lucide-react';
import { cn } from '../ui/utils';
import { useModelContext } from '@contexts/ModelContext';

export function ModelSelector() {
  const { models, selectedModelId, selectModel, openCustomModelModal, loading } = useModelContext();
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

  const selectedModel = models.find((m) => m.model_id === selectedModelId);
  const displayName = selectedModel?.display_name || selectedModelId || '选择模型';

  if (loading && models.length === 0) {
    return (
      <div
        className="flex h-7 items-center gap-1.5 rounded-lg border border-border bg-secondary px-2.5"
        aria-busy="true"
        aria-label="加载模型列表"
      >
        <Sparkles className="size-[13px] animate-pulse text-muted-foreground" />
        <span className="text-xs font-semibold text-muted-foreground">加载中...</span>
      </div>
    );
  }

  return (
    <div className="relative" ref={ref} data-testid="model-selector-root">
      {/* 触发器 */}
      <button
        type="button"
        onClick={() => setOpen((prev) => !prev)}
        className={cn(
          'flex h-7 items-center gap-1.5 rounded-lg border border-border',
          'bg-secondary px-2.5 text-xs font-semibold',
          'transition-colors hover:bg-accent',
          open && 'bg-accent',
        )}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-label={`当前模型：${displayName}`}
        data-testid="model-selector-trigger"
      >
        <Sparkles className="size-[13px] text-primary" />
        <span className="max-w-[120px] truncate text-foreground">{displayName}</span>
        <ChevronDown
          className={cn(
            'size-[13px] text-muted-foreground transition-transform duration-150',
            open && 'rotate-180',
          )}
        />
      </button>

      {/* 下拉面板 */}
      {open && (
        <div
          role="listbox"
          aria-label="模型列表"
          className={cn(
            'absolute bottom-full left-0 z-50 mb-1.5',
            'w-60 rounded-xl border border-border bg-popover py-1.5',
            'shadow-[0_8px_24px_rgba(0,0,0,0.09)]',
          )}
          data-testid="model-selector-panel"
        >
          {/* 标题 */}
          <div className="px-3 py-1.5">
            <span className="text-[11px] font-semibold tracking-[0.5px] text-muted-foreground">
              选择模型
            </span>
          </div>

          {/* 模型选项 */}
          {models.map((model) => {
            const selected = selectedModelId === model.model_id;
            return (
              <button
                key={model.model_id}
                type="button"
                role="option"
                aria-selected={selected}
                onClick={() => {
                  selectModel(model.model_id);
                  setOpen(false);
                }}
                className={cn(
                  'flex h-[38px] w-[calc(100%-12px)] items-center justify-between',
                  'mx-1.5 rounded-lg px-3 transition-colors',
                  selected ? 'bg-[#F0F9F4] dark:bg-primary/10' : 'hover:bg-accent',
                )}
                data-testid={`model-option-${model.model_id}`}
              >
                <div className="flex min-w-0 items-center gap-2">
                  <Sparkles
                    className={cn(
                      'size-[13px] shrink-0',
                      selected ? 'text-primary' : 'text-muted-foreground',
                    )}
                  />
                  <span
                    className={cn(
                      'truncate text-[13px]',
                      selected ? 'font-semibold text-foreground' : 'font-medium text-[#4A4947] dark:text-foreground',
                    )}
                  >
                    {model.display_name}
                  </span>
                  {model.is_default && (
                    <span className="shrink-0 rounded-full bg-[#C8F0D8] px-1.5 py-0.5 text-[10px] font-medium text-primary dark:bg-primary/20">
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
          <div className="mx-3 my-1.5 h-px bg-[#EEEDE9] dark:bg-border" />

          {/* 自定义模型入口 */}
          <button
            type="button"
            onClick={() => {
              setOpen(false);
              openCustomModelModal();
            }}
            className={cn(
              'flex h-[38px] w-[calc(100%-12px)] items-center justify-between',
              'mx-1.5 rounded-lg px-3 transition-colors hover:bg-accent',
            )}
            data-testid="model-selector-custom"
          >
            <div className="flex items-center gap-2">
              <CirclePlus className="size-[13px] text-[#6D6C6A] dark:text-muted-foreground" />
              <span className="text-[13px] font-medium text-[#4A4947] dark:text-foreground">
                自定义模型
              </span>
            </div>
            <ChevronRight className="size-[13px] text-[#B0AFAC] dark:text-muted-foreground" />
          </button>
        </div>
      )}
    </div>
  );
}
