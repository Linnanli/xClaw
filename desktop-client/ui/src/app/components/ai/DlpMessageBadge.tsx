/**
 * DlpMessageBadge - 消息级 DLP 状态标记
 *
 * 在已脱敏的用户消息气泡下方显示小型标记，
 * 提示该消息经过 DLP 处理。
 *
 * 设计规范：
 * - 使用 shadcn/ui Badge + Tooltip
 * - amber 色调，ShieldCheck 图标
 * - hover 显示详细统计
 */

import { ShieldCheck } from 'lucide-react';
import { Badge } from '../ui/badge';
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '../ui/tooltip';
import { cn } from '../ui/utils';
import type { SanitizationStats } from '../../hooks/useDlpScan';

export interface DlpMessageBadgeProps {
  /** 脱敏统计 */
  stats: SanitizationStats;
  className?: string;
}

export function DlpMessageBadge({ stats, className }: DlpMessageBadgeProps) {
  if (stats.redacted_count === 0 && stats.warned_count === 0) {
    return null;
  }

  return (
    <TooltipProvider>
      <Tooltip>
        <TooltipTrigger asChild>
          <Badge
            variant="secondary"
            className={cn(
              'cursor-help gap-1 bg-amber-100 px-1.5 py-0 text-[10px] text-amber-700',
              'dark:bg-amber-900/30 dark:text-amber-300',
              className,
            )}
          >
            <ShieldCheck className="size-3" />
            已脱敏
          </Badge>
        </TooltipTrigger>
        <TooltipContent side="bottom" className="text-xs">
          <div className="space-y-0.5">
            <p className="font-medium">DLP 数据保护</p>
            {stats.redacted_count > 0 && (
              <p>脱敏处理：{stats.redacted_count} 处</p>
            )}
            {stats.warned_count > 0 && (
              <p>安全提醒：{stats.warned_count} 处</p>
            )}
            <p className="text-muted-foreground">
              共检测 {stats.total_matches} 处敏感信息
            </p>
          </div>
        </TooltipContent>
      </Tooltip>
    </TooltipProvider>
  );
}
