/**
 * AppSidebar - 应用侧边栏导航
 *
 * 基于设计稿实现：240px 宽度，顶部 Logo + 新建聊天按钮，
 * 中间导航项 + 聊天历史，底部日志/定时任务/设置/用户。
 * 复用 shadcn/ui Sidebar 组件体系。
 */

import { useState, useEffect } from 'react';
import {
  MessageCircle,
  Plus,
  FileText,
  Timer,
  Settings,
  Shield,
  ChevronDown,
  ChevronRight,
} from 'lucide-react';
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarSeparator,
} from '../ui/sidebar';
import { Button } from '../ui/button';
import { ScrollArea } from '../ui/scroll-area';
import { Tooltip, TooltipContent, TooltipTrigger } from '../ui/tooltip';
import { cn } from '../ui/utils';
import { threadApi, type Thread } from '../../utils/tauri';

export type NavItem = 'chat' | 'logs' | 'routines' | 'settings';

interface AppSidebarProps {
  activeNav: NavItem;
  onNavChange: (nav: NavItem) => void;
  selectedThreadId: string | null;
  onThreadSelect: (threadId: string) => void;
  onNewChat: () => void;
}

interface GroupedThreads {
  today: Thread[];
  yesterday: Thread[];
  older: Thread[];
}

function groupThreadsByDate(threads: Thread[]): GroupedThreads {
  const now = new Date();
  const todayStart = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const yesterdayStart = new Date(todayStart.getTime() - 86400000);

  const groups: GroupedThreads = { today: [], yesterday: [], older: [] };

  for (const thread of threads) {
    const created = new Date(thread.created_at || Date.now());
    if (created >= todayStart) {
      groups.today.push(thread);
    } else if (created >= yesterdayStart) {
      groups.yesterday.push(thread);
    } else {
      groups.older.push(thread);
    }
  }
  return groups;
}

export function AppSidebar({
  activeNav,
  onNavChange,
  selectedThreadId,
  onThreadSelect,
  onNewChat,
}: AppSidebarProps) {
  const [threads, setThreads] = useState<Thread[]>([]);
  const [expandedGroups, setExpandedGroups] = useState({
    today: true,
    yesterday: true,
    older: false,
  });

  useEffect(() => {
    loadThreads();
  }, []);

  const loadThreads = async () => {
    try {
      const response = await threadApi.getThreads();
      const list = Array.isArray(response) ? response : response.threads || [];
      setThreads(list);
    } catch (err) {
      console.error('Failed to load threads:', err);
    }
  };

  const grouped = groupThreadsByDate(threads);

  const toggleGroup = (group: keyof typeof expandedGroups) => {
    setExpandedGroups((prev) => ({ ...prev, [group]: !prev[group] }));
  };

  const renderThreadGroup = (
    label: string,
    groupKey: keyof typeof expandedGroups,
    items: Thread[],
  ) => {
    if (items.length === 0) return null;
    return (
      <div key={groupKey}>
        <button
          onClick={() => toggleGroup(groupKey)}
          className="flex w-full items-center px-3 py-1 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground hover:text-foreground"
        >
          {expandedGroups[groupKey] ? (
            <ChevronDown className="mr-1 size-3" />
          ) : (
            <ChevronRight className="mr-1 size-3" />
          )}
          {label}
        </button>
        {expandedGroups[groupKey] &&
          items.map((thread) => (
            <button
              key={thread.id}
              onClick={() => {
                onThreadSelect(thread.id);
                onNavChange('chat');
              }}
              className={cn(
                'flex w-full items-center gap-2 rounded-lg px-3 py-2 text-[13px] transition-colors',
                selectedThreadId === thread.id
                  ? 'bg-sidebar-accent text-sidebar-accent-foreground'
                  : 'text-text-secondary hover:bg-sidebar-accent/50',
              )}
            >
              <MessageCircle className="size-3.5 shrink-0 text-muted-foreground" />
              <span className="truncate">{thread.title || '新对话'}</span>
            </button>
          ))}
      </div>
    );
  };

  return (
    <Sidebar collapsible="none" className="border-r border-sidebar-border">
      {/* Logo + New Chat */}
      <SidebarHeader className="h-14 flex-row items-center justify-between px-5">
        <div className="flex items-center gap-2.5">
          <Shield className="size-5 text-primary" />
          <span className="text-base font-bold text-foreground">X-Claw</span>
        </div>
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant="outline"
              size="icon"
              className="size-7 border-primary/20 bg-primary/5 hover:bg-primary/10"
              onClick={onNewChat}
              aria-label="新建聊天"
            >
              <Plus className="size-4 text-primary" />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="right">新建聊天</TooltipContent>
        </Tooltip>
      </SidebarHeader>

      {/* Main Nav + Chat History */}
      <SidebarContent>
        <SidebarGroup className="px-3 pt-2">
          <SidebarMenu>
            <SidebarMenuItem>
              <SidebarMenuButton
                isActive={activeNav === 'chat'}
                onClick={() => onNavChange('chat')}
                className={cn(
                  'rounded-[10px] font-semibold',
                  activeNav === 'chat' &&
                    'bg-primary text-primary-foreground hover:bg-primary/90 hover:text-primary-foreground',
                )}
              >
                <MessageCircle className="size-[18px]" />
                <span>聊天</span>
              </SidebarMenuButton>
            </SidebarMenuItem>
          </SidebarMenu>
        </SidebarGroup>

        {/* Chat History */}
        <SidebarGroup className="flex-1 px-3">
          <ScrollArea className="h-full">
            <SidebarGroupContent className="space-y-1">
              {renderThreadGroup('今天', 'today', grouped.today)}
              {renderThreadGroup('昨天', 'yesterday', grouped.yesterday)}
              {renderThreadGroup('更早', 'older', grouped.older)}
            </SidebarGroupContent>
          </ScrollArea>
        </SidebarGroup>
      </SidebarContent>

      {/* Bottom Nav */}
      <SidebarSeparator />
      <SidebarFooter className="px-3 pb-3">
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton
              isActive={activeNav === 'logs'}
              onClick={() => onNavChange('logs')}
              className="rounded-[10px]"
            >
              <FileText className="size-[18px]" />
              <span>日志</span>
            </SidebarMenuButton>
          </SidebarMenuItem>
          <SidebarMenuItem>
            <SidebarMenuButton
              isActive={activeNav === 'routines'}
              onClick={() => onNavChange('routines')}
              className="rounded-[10px]"
            >
              <Timer className="size-[18px]" />
              <span>定时任务</span>
            </SidebarMenuButton>
          </SidebarMenuItem>
          <SidebarMenuItem>
            <SidebarMenuButton
              isActive={activeNav === 'settings'}
              onClick={() => onNavChange('settings')}
              className="rounded-[10px]"
            >
              <Settings className="size-[18px]" />
              <span>设置</span>
            </SidebarMenuButton>
          </SidebarMenuItem>
          <SidebarMenuItem>
            <div className="flex items-center gap-2.5 rounded-[10px] px-2 py-2">
              <div className="flex size-8 items-center justify-center rounded-full bg-primary text-xs font-bold text-primary-foreground">
                U
              </div>
              <span className="text-sm font-medium text-foreground">用户</span>
            </div>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarFooter>
    </Sidebar>
  );
}
