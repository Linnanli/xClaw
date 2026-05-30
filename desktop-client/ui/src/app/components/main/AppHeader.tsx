/**
 * AppHeader - 主内容区顶部栏
 *
 * 设计稿：56px 高度，左侧面包屑标题，右侧任务状态 + 通知铃铛 + 主题切换。
 */

import { Bell, Briefcase, Loader2 } from 'lucide-react';
import { Button } from '../ui/button';
import { cn } from '../ui/utils';
import { ThemeToggle } from './ThemeToggle';
import { type ThemeMode } from '../../contexts/ThemeContext';

interface AppHeaderProps {
  title: string;
  /** 正在运行的任务数 */
  runningJobs?: number;
  /** 未读通知数 */
  unreadCount?: number;
  onJobsClick?: () => void;
  onNotificationsClick?: () => void;
  /** 当前主题模式 */
  theme?: ThemeMode;
  /** 主题切换回调 */
  onThemeChange?: (mode: ThemeMode) => void;
  className?: string;
}

export function AppHeader({
  title,
  runningJobs = 0,
  unreadCount = 0,
  onJobsClick,
  onNotificationsClick,
  theme = 'system',
  onThemeChange,
  className,
}: AppHeaderProps) {
  return (
    <header
      className={cn(
        'flex h-14 items-center justify-between border-b border-border bg-background px-6',
        className,
      )}
    >
      {/* Left: breadcrumb / title */}
      <div className="flex items-center gap-2">
        <h1 className="text-base font-bold text-foreground">{title}</h1>
      </div>

      {/* Right: actions */}
      <div className="flex items-center gap-3">
        {/* Jobs button */}
        {runningJobs > 0 ? (
          <button
            onClick={onJobsClick}
            className="flex h-8 items-center gap-1.5 rounded-lg border border-[#E5E4E1] bg-[#F5F4F1] px-3 text-xs font-medium text-[#3D8A5A]"
          >
            <Loader2 className="size-3.5 animate-spin text-[#3D8A5A]" />
            <span>{runningJobs} 个任务运行中</span>
            <span className="size-1.5 rounded-full bg-[#3D8A5A]" />
          </button>
        ) : (
          <Button
            variant="outline"
            size="sm"
            className="h-8 gap-1.5 rounded-lg border-border bg-secondary text-xs font-medium"
            onClick={onJobsClick}
          >
            <Briefcase className="size-3.5" />
            <span>任务</span>
          </Button>
        )}

        {/* Notification bell */}
        <Button
          variant="outline"
          size="icon"
          className="relative size-8 rounded-lg border-border bg-secondary"
          onClick={onNotificationsClick}
          aria-label="通知"
        >
          <Bell className="size-4" />
          {unreadCount > 0 && (
            <span className="absolute -right-0.5 -top-0.5 flex size-2 items-center justify-center rounded-full bg-[#D94040]" />
          )}
        </Button>

        {/* Theme toggle */}
        {onThemeChange && (
          <ThemeToggle theme={theme} onThemeChange={onThemeChange} />
        )}
      </div>
    </header>
  );
}
