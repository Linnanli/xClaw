/**
 * QuickActions - 快捷操作按钮组
 *
 * Pencil 设计稿规范：
 * - 药丸按钮：cornerRadius:100, fill:#FFFFFF, gap:6, padding:[8,16]
 * - 边框：stroke:#E5E4E1 1px
 * - 文字：fontSize:13, fontWeight:500
 * - 图标：fill:#3D8A5A, 14x14
 * - 容器间距：gap:10
 */

import type { LucideIcon } from 'lucide-react';
import { cn } from '../ui/utils';

export interface QuickAction {
  icon: LucideIcon;
  label: string;
  prompt: string;
}

export interface QuickActionsProps {
  actions: QuickAction[];
  onSelect: (prompt: string) => void;
  className?: string;
}

export function QuickActions({ actions, onSelect, className }: QuickActionsProps) {
  return (
    <div className={cn('flex flex-wrap items-center justify-center gap-2.5', className)}>
      {actions.map((action) => (
        <button
          key={action.label}
          onClick={() => onSelect(action.prompt)}
          className={cn(
            'flex items-center gap-1.5 rounded-full',
            'border border-border bg-card px-4 py-2',
            'text-[13px] font-medium text-foreground',
            'transition-colors hover:bg-secondary',
          )}
        >
          <action.icon className="size-3.5 text-primary" />
          {action.label}
        </button>
      ))}
    </div>
  );
}
