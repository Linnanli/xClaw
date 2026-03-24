/**
 * DlpWarningBanner - DLP 敏感信息警告横幅
 *
 * 使用 shadcn/ui Alert 组件，在聊天区域内联显示 DLP 扫描结果。
 * 支持两种场景：
 * - 脱敏警告（warning）：检测到敏感信息并已脱敏
 * - 阻止警告（destructive）：检测到高危信息，消息被阻止
 *
 * 设计规范：
 * - 脱敏：amber 色调，ShieldAlert 图标
 * - 阻止：destructive 色调，ShieldX 图标
 * - 自动 5s 后淡出（可配置）
 * - 支持手动关闭
 */

import { useEffect, useState } from 'react';
import { ShieldAlert, ShieldX, X } from 'lucide-react';
import { Alert, AlertTitle, AlertDescription } from '../ui/alert';
import { Badge } from '../ui/badge';
import { Button } from '../ui/button';
import { cn } from '../ui/utils';
import type { SanitizationStats } from '../../hooks/useDlpScan';

export interface DlpWarningBannerProps {
  /** 警告类型 */
  type: 'redacted' | 'blocked';
  /** 脱敏统计 */
  stats: SanitizationStats;
  /** 阻止原因（仅 blocked 类型） */
  blockReason?: string;
  /** 关闭回调 */
  onClose: () => void;
  /** 自动关闭延迟（ms），0 表示不自动关闭，默认 5000 */
  autoCloseDelay?: number;
  className?: string;
}

export function DlpWarningBanner({
  type,
  stats,
  blockReason,
  onClose,
  autoCloseDelay = 5000,
  className,
}: DlpWarningBannerProps) {
  const [visible, setVisible] = useState(true);

  useEffect(() => {
    if (autoCloseDelay <= 0) return;

    const timer = setTimeout(() => {
      setVisible(false);
      // 等待淡出动画完成后触发 onClose
      setTimeout(onClose, 300);
    }, autoCloseDelay);

    return () => clearTimeout(timer);
  }, [autoCloseDelay, onClose]);

  const handleClose = () => {
    setVisible(false);
    setTimeout(onClose, 300);
  };

  const isBlocked = type === 'blocked';

  return (
    <div
      className={cn(
        'transition-all duration-300',
        visible ? 'opacity-100 translate-y-0' : 'opacity-0 -translate-y-2',
        className,
      )}
      role="alert"
      aria-live="polite"
    >
      <Alert
        variant={isBlocked ? 'destructive' : 'default'}
        className={cn(
          'relative',
          !isBlocked && 'border-amber-200 bg-amber-50 text-amber-900 dark:border-amber-800 dark:bg-amber-950 dark:text-amber-100',
        )}
      >
        {isBlocked ? (
          <ShieldX className="size-4" />
        ) : (
          <ShieldAlert className="size-4 text-amber-600 dark:text-amber-400" />
        )}

        <AlertTitle className="flex items-center gap-2">
          {isBlocked ? '消息已被阻止' : '检测到敏感信息'}
          {stats.total_matches > 0 && (
            <Badge
              variant={isBlocked ? 'destructive' : 'secondary'}
              className={cn(
                'text-[10px] px-1.5 py-0',
                !isBlocked && 'bg-amber-200 text-amber-800 dark:bg-amber-800 dark:text-amber-200',
              )}
            >
              {stats.total_matches} 处
            </Badge>
          )}
        </AlertTitle>

        <AlertDescription>
          {isBlocked ? (
            <p>{blockReason || '消息包含高危敏感信息，已被安全策略阻止发送。'}</p>
          ) : (
            <div className="space-y-1">
              {stats.redacted_count > 0 && (
                <p>已自动脱敏 {stats.redacted_count} 处敏感信息</p>
              )}
              {stats.warned_count > 0 && (
                <p>发现 {stats.warned_count} 处潜在敏感信息</p>
              )}
            </div>
          )}
        </AlertDescription>

        {/* 关闭按钮 */}
        <Button
          variant="ghost"
          size="icon"
          className="absolute right-2 top-2 size-6 text-current opacity-70 hover:opacity-100"
          onClick={handleClose}
          aria-label="关闭 DLP 警告"
        >
          <X className="size-3.5" />
        </Button>
      </Alert>
    </div>
  );
}
