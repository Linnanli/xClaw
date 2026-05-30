/**
 * ThinkingProcess - AI 思考过程展示组件
 *
 * 复用 assistant-ui 的 Reasoning 子组件（ReasoningRoot/Trigger/Content/Text），
 * 保持与 thread.tsx 中 reasoning part 渲染一致的视觉语言。
 *
 * 视觉规范（设计稿 client-design.pen → X-Claw Thinking Process / Thinking Complete）：
 * - AI 头像：#2D6B45 32×32，fallback "XC"
 * - 步骤编号：16×16 圆形, bg #F0EFED, fontSize 10, fill #9D9C9A
 * - 步骤文字：fontSize 12, fill #6B6A68
 */

import { useEffect, useState } from 'react';
import { WrenchIcon } from 'lucide-react';
import { Avatar, AvatarFallback } from '../ui/avatar';
import {
  ReasoningRoot,
  ReasoningContent,
  ReasoningText,
} from '../assistant-ui/reasoning';
import {
  CollapsibleTrigger,
} from '../ui/collapsible';
import { cn } from '../ui/utils';
import type { ThinkingStep } from '../../hooks/useAiChatTauri';

export interface ThinkingProcessProps {
  steps: ThinkingStep[];
  isActive: boolean;
  hideAvatar?: boolean;
  className?: string;
}

export function ThinkingProcess({ steps, isActive, hideAvatar, className }: ThinkingProcessProps) {
  const [isOpen, setIsOpen] = useState(false);

  useEffect(() => {
    setIsOpen(isActive);
  }, [isActive]);

  if (steps.length === 0 && !isActive) return null;

  const latestStep = steps[steps.length - 1];
  const triggerLabel = isActive
    ? (latestStep?.message ?? '正在思考...')
    : `思考完成（${steps.length} 步）`;

  return (
    <div className={cn('flex flex-col items-start gap-1.5', className)}>
      {!hideAvatar && (
        <div className="flex items-center gap-2.5">
          <Avatar className="size-8">
            <AvatarFallback className="bg-[#2D6B45] text-xs font-medium text-white">
              XC
            </AvatarFallback>
          </Avatar>
          <span className="text-[11px] font-semibold text-[#9D9C9A]">X-Claw</span>
        </div>
      )}

      <div className={cn('w-full', !hideAvatar && 'pl-[42px]')}>
        <ReasoningRoot
          open={isOpen}
          onOpenChange={setIsOpen}
          variant="outline"
        >
          <CollapsibleTrigger
            data-slot="reasoning-trigger"
            className="aui-reasoning-trigger group/trigger flex max-w-full items-center gap-2 py-1 text-muted-foreground text-sm transition-colors hover:text-foreground w-full"
          >
            <WrenchIcon
              data-slot="reasoning-trigger-icon"
              className="aui-reasoning-trigger-icon size-4 shrink-0"
            />
            <span
              data-slot="reasoning-trigger-label"
              className="aui-reasoning-trigger-label-wrapper relative inline-block leading-none flex-1 text-left truncate"
            >
              <span>{triggerLabel}</span>
              {isActive && (
                <span
                  aria-hidden
                  data-slot="reasoning-trigger-shimmer"
                  className="aui-reasoning-trigger-shimmer shimmer pointer-events-none absolute inset-0 motion-reduce:animate-none"
                >
                  {triggerLabel}
                </span>
              )}
            </span>
          </CollapsibleTrigger>

          <ReasoningContent aria-busy={isActive}>
            <ReasoningText>
              <ol className="space-y-1.5" aria-label="思考步骤">
                {steps.map((step, index) => (
                  <li key={step.id} className="flex items-start gap-2 text-xs text-[#6B6A68]">
                    <span className="mt-0.5 flex size-4 shrink-0 items-center justify-center rounded-full bg-[#F0EFED] text-[10px] font-medium text-[#9D9C9A]">
                      {index + 1}
                    </span>
                    <span className="leading-relaxed">{step.message}</span>
                  </li>
                ))}
                {isActive && (
                  <li className="flex items-center gap-2 text-xs text-[#9D9C9A]" aria-label="等待下一步">
                    <span className="mt-0.5 flex size-4 shrink-0 items-center justify-center">
                      <span className="inline-block size-1 animate-pulse rounded-full bg-[#2D6B45]" />
                    </span>
                    <span className="animate-pulse">...</span>
                  </li>
                )}
              </ol>
            </ReasoningText>
          </ReasoningContent>
        </ReasoningRoot>
      </div>
    </div>
  );
}
