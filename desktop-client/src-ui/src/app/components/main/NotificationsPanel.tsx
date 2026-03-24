/**
 * NotificationsPanel - 通知面板（右侧抽屉）
 *
 * 设计稿：380px 右侧面板，标签页（全部/系统/审批/任务）+ 通知列表。
 * 不同类型通知有不同背景色。
 */

import { useState } from 'react';
import { Bell, Monitor, ClipboardCheck, Briefcase } from 'lucide-react';
import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
  SheetDescription,
} from '../ui/sheet';
import { Tabs, TabsList, TabsTrigger, TabsContent } from '../ui/tabs';
import { ScrollArea } from '../ui/scroll-area';
import { cn } from '../ui/utils';

interface Notification {
  id: string;
  type: 'system' | 'approval' | 'task';
  title: string;
  message: string;
  time: string;
  read: boolean;
}

interface NotificationsPanelProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  notifications?: Notification[];
}

const TYPE_STYLES: Record<Notification['type'], { bg: string; icon: React.ElementType; label: string }> = {
  system: { bg: 'bg-emerald-50 dark:bg-emerald-950/30', icon: Monitor, label: '系统' },
  approval: { bg: 'bg-amber-50 dark:bg-amber-950/30', icon: ClipboardCheck, label: '审批' },
  task: { bg: 'bg-card', icon: Briefcase, label: '任务' },
};

const SAMPLE_NOTIFICATIONS: Notification[] = [
  { id: '1', type: 'system', title: '系统更新', message: '新版本 0.2.0 已发布，请及时更新。', time: '5 分钟前', read: false },
  { id: '2', type: 'approval', title: '审批请求', message: '张三请求访问敏感数据，请审批。', time: '15 分钟前', read: false },
  { id: '3', type: 'task', title: '任务完成', message: '代码审查任务已完成。', time: '1 小时前', read: true },
  { id: '4', type: 'system', title: '安全提醒', message: 'DLP 规则已更新，新增 3 条检测规则。', time: '2 小时前', read: true },
];

export function NotificationsPanel({
  open,
  onOpenChange,
  notifications = SAMPLE_NOTIFICATIONS,
}: NotificationsPanelProps) {
  const [activeTab, setActiveTab] = useState('all');

  const filtered =
    activeTab === 'all'
      ? notifications
      : notifications.filter((n) => n.type === activeTab);

  const unreadCount = notifications.filter((n) => !n.read).length;

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent side="right" className="w-[380px] p-0 sm:max-w-[380px]">
        {/* Header */}
        <SheetHeader className="border-b border-border px-5 py-4">
          <div className="flex items-center gap-2">
            <Bell className="size-4 text-foreground" />
            <SheetTitle className="text-base">通知中心</SheetTitle>
            {unreadCount > 0 && (
              <span className="flex size-5 items-center justify-center rounded-full bg-destructive text-[10px] font-bold text-destructive-foreground">
                {unreadCount}
              </span>
            )}
          </div>
          <SheetDescription className="sr-only">查看通知</SheetDescription>
        </SheetHeader>

        {/* Tabs */}
        <Tabs value={activeTab} onValueChange={setActiveTab} className="flex flex-col">
          <div className="border-b border-border px-4 pt-2">
            <TabsList className="w-full">
              <TabsTrigger value="all" className="flex-1 text-xs">全部</TabsTrigger>
              <TabsTrigger value="system" className="flex-1 text-xs">系统</TabsTrigger>
              <TabsTrigger value="approval" className="flex-1 text-xs">审批</TabsTrigger>
              <TabsTrigger value="task" className="flex-1 text-xs">任务</TabsTrigger>
            </TabsList>
          </div>

          <TabsContent value={activeTab} className="mt-0 flex-1">
            <ScrollArea className="h-[calc(100vh-180px)]">
              <div className="space-y-2 p-4">
                {filtered.length === 0 && (
                  <div className="py-12 text-center text-sm text-muted-foreground">
                    <Bell className="mx-auto mb-3 size-8 opacity-40" />
                    暂无通知
                  </div>
                )}
                {filtered.map((n) => {
                  const style = TYPE_STYLES[n.type];
                  const Icon = style.icon;
                  return (
                    <div
                      key={n.id}
                      className={cn(
                        'rounded-xl border border-border p-3 transition-colors',
                        n.read ? 'bg-card' : style.bg,
                      )}
                    >
                      <div className="flex items-start gap-3">
                        <div className="mt-0.5 flex size-8 shrink-0 items-center justify-center rounded-lg bg-primary/10">
                          <Icon className="size-4 text-primary" />
                        </div>
                        <div className="flex-1">
                          <div className="flex items-center justify-between">
                            <p className="text-sm font-medium text-foreground">
                              {n.title}
                            </p>
                            {!n.read && (
                              <span className="size-2 rounded-full bg-primary" />
                            )}
                          </div>
                          <p className="mt-0.5 text-xs text-muted-foreground line-clamp-2">
                            {n.message}
                          </p>
                          <p className="mt-1 text-[11px] text-text-tertiary">
                            {n.time}
                          </p>
                        </div>
                      </div>
                    </div>
                  );
                })}
              </div>
            </ScrollArea>
          </TabsContent>
        </Tabs>
      </SheetContent>
    </Sheet>
  );
}
