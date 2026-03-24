/**
 * ThinkingProcess - AI 思考过程展示组件
 *
 * 使用 shadcn/ui Collapsible 实现可折叠的思考步骤列表。
 * 思考进行中时自动展开并显示脉冲指示器；完成后折叠，可手动展开查看。
 *
 * 视觉规范（设计稿 client-design.pen → X-Claw Thinking Process / Thinking Complete）：
 * - AI 头像：#2D6B45 32×32，fallback "XC"
 * - 气泡：#FFFFFF 背景 + #E5E4E1 1px 边框，圆角 [4,16,16,16]
 * - 气泡偏移 42px（头像 32px + gap 10px）
 * - 名称标签：fontSize 11, fontWeight 600, fill #9D9C9A, fontFamily Outfit
 * - 触发器：padding [12,16], gap 8, fontSize 14, fill #4A4947
 * - 脉冲指示器：8×8, active=#2D6B45(带 ping), inactive=#9D9C9A
 * - 步骤区域：padding [8,16,12,16], gap 6, border-top #E5E4E1
 * - 步骤编号：16×16 圆形, bg #F0EFED, fontSize 10, fill #9D9C9A
 * - 步骤文字：fontSize 12, fill #6B6A68
 * - 等待指示：4×4 圆点 #2D6B45 + "..." #9D9C9A
 */

import { useEffect, useState } from 'react';
import { ChevronDown } from 'lucide-react';
import { cn } from '../ui/utils';
import { Avatar, AvatarFallback } from '../ui/avatar';
import {
  Collapsible,
  CollapsibleTrigger,
  CollapsibleContent,
} from '../ui/collapsible';
import type { ThinkingStep } from '../../hooks/useAiChatTauri';

export interface ThinkingProcessProps {
  /** 思考步骤列表 */
  steps: ThinkingStep[];
  /** 是否正在思考中（控制脉冲动画和自动展开） */
  isActive: boolean;
  className?: string;
}

export function ThinkingProcess({ steps, isActive, className }: ThinkingProcessProps) {
  const [isOpen, setIsOpen] = useState(false);

  // 思考进行中时自动展开，完成后自动折叠
  useEffect(() => {
    setIsOpen(isActive);
  }, [isActive]);

  if (steps.length === 0 && !isActive) return null;

  const latestStep = steps[steps.length - 1];

  return (
    <div className={cn('flex flex-col items-start gap-1.5', className)}>
      {/* AI 头像 + 名称 */}
      <div className="flex items-center gap-2.5">
        <Avatar className="size-8">
          <AvatarFallback className="bg-[#2D6B45] text-xs font-medium text-white">
            XC
          </AvatarFallback>
        </Avatar>
        <span className="text-[11px] font-semibold text-[#9D9C9A]">X-Claw</span>
      </div>

      {/* 思考气泡（偏移 42px 对齐头像下方，设计稿 thinkingWrap padding [0,0,0,42]） */}
      <div className="w-full pl-[42px]">
        <Collapsible open={isOpen} onOpenChange={setIsOpen}>
          <div className="overflow-hidden rounded-tl rounded-tr-2xl rounded-br-2xl rounded-bl-2xl border border-[#E5E4E1] bg-white">
            {/* 触发器：设计稿 trigger padding [12,16] gap 8 */}
            <CollapsibleTrigger asChild>
              <button
                type="button"
                className="flex w-full items-center gap-2 px-4 py-3 text-left text-sm text-[#4A4947] hover:bg-[#FAFAF9] transition-colors"
                aria-label={isOpen ? '收起思考过程' : '展开思考过程'}
              >
                {/* 脉冲指示器 */}
                {isActive ? (
                  <span className="relative flex size-2 shrink-0" aria-hidden="true">
                    <span className="absolute inline-flex size-full animate-ping rounded-full bg-[#2D6B45] opacity-75" />
                    <span className="relative inline-flex size-2 rounded-full bg-[#2D6B45]" />
                  </span>
                ) : (
                  <span className="flex size-2 shrink-0 rounded-full bg-[#9D9C9A]" aria-hidden="true" />
                )}

                <span className="flex-1 truncate">
                  {isActive
                    ? latestStep?.message || '正在思考...'
                    : `思考完成（${steps.length} 步）`}
                </span>

                <ChevronDown
                  className={cn(
                    'size-4 shrink-0 text-[#9D9C9A] transition-transform duration-200',
                    isOpen && 'rotate-180',
                  )}
                  aria-hidden="true"
                />
              </button>
            </CollapsibleTrigger>

            {/* 展开内容：设计稿 stepsContent padding [8,16,12,16] gap 6 border-top */}
            <CollapsibleContent>
              <div className="border-t border-[#E5E4E1] px-4 pt-2 pb-3">
                <ol className="space-y-1.5" aria-label="思考步骤">
                  {steps.map((step, index) => (
                    <li
                      key={step.id}
                      className="flex items-start gap-2 text-xs text-[#6B6A68]"
                    >
                      <span className="mt-0.5 flex size-4 shrink-0 items-center justify-center rounded-full bg-[#F0EFED] text-[10px] font-medium text-[#9D9C9A]">
                        {index + 1}
                      </span>
                      <span className="leading-relaxed">{step.message}</span>
                    </li>
                  ))}
                  {/* 活跃状态下的等待指示 */}
                  {isActive && (
                    <li className="flex items-center gap-2 text-xs text-[#9D9C9A]" aria-label="等待下一步">
                      <span className="mt-0.5 flex size-4 shrink-0 items-center justify-center">
                        <span className="inline-block size-1 animate-pulse rounded-full bg-[#2D6B45]" />
                      </span>
                      <span className="animate-pulse">...</span>
                    </li>
                  )}
                </ol>
              </div>
            </CollapsibleContent>
          </div>
        </Collapsible>
      </div>
    </div>
  );
}
