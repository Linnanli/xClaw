/**
 * RoutinesTab - 定时任务管理
 *
 * 设计稿：960x680 模态框，左侧 200px 导航（全部/已启用/已禁用）+ 右侧任务卡片列表。
 * 每张卡片包含：名称 + 标签 + 开关 + 运行按钮 + 描述 + 元信息（触发方式/上次执行/执行次数）。
 */

import { useState, useEffect } from 'react';
import {
  Plus,
  List,
  CirclePlay,
  CirclePause,
  Clock3,
  CircleCheck,
  Hash,
  Play,
} from 'lucide-react';
import {
  Dialog,
  DialogContent,
  DialogTitle,
  DialogDescription,
} from '../ui/dialog';
import { Switch } from '../ui/switch';
import { ScrollArea } from '../ui/scroll-area';
import { Button } from '../ui/button';
import { cn } from '../ui/utils';
import { routineApi, routineExtendedApi, type Routine } from '../../utils/tauri';

type FilterKey = 'all' | 'enabled' | 'disabled';

interface RoutinesTabProps {
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
}

const TRIGGER_LABELS: Record<string, string> = {
  manual: '手动触发',
  time: '时间触发',
  event: '事件触发',
  Manual: '手动触发',
  Time: '时间触发',
  Event: '事件触发',
};

const TAG_COLORS: Record<string, { bg: string; text: string }> = {
  time: { bg: 'bg-blue-50', text: 'text-blue-600' },
  event: { bg: 'bg-amber-50', text: 'text-amber-600' },
  manual: { bg: 'bg-secondary', text: 'text-muted-foreground' },
};

export function RoutinesTab({ open = true, onOpenChange }: RoutinesTabProps) {
  const [filter, setFilter] = useState<FilterKey>('all');
  const [routines, setRoutines] = useState<Routine[]>([]);
  const [loading, setLoading] = useState(false);
  const [showCreate, setShowCreate] = useState(false);
  const [newName, setNewName] = useState('');
  const [newDesc, setNewDesc] = useState('');
  const [newTrigger, setNewTrigger] = useState<'manual' | 'time' | 'event'>('manual');
  const [newCron, setNewCron] = useState('');

  useEffect(() => {
    if (open) loadRoutines();
  }, [open]);

  const loadRoutines = async () => {
    setLoading(true);
    try {
      const data = await routineApi.getRoutines();
      setRoutines(data);
    } catch (err) {
      console.error('Failed to load routines:', err);
    } finally {
      setLoading(false);
    }
  };

  const filtered = routines.filter((r) => {
    if (filter === 'enabled') return r.status === 'active';
    if (filter === 'disabled') return r.status !== 'active';
    return true;
  });

  const counts = {
    all: routines.length,
    enabled: routines.filter((r) => r.status === 'active').length,
    disabled: routines.filter((r) => r.status !== 'active').length,
  };

  const handleToggle = async (id: string, currentStatus: string) => {
    try {
      if (currentStatus === 'active') {
        await routineExtendedApi.pauseRoutine(id);
      } else {
        await routineExtendedApi.enableRoutine(id);
      }
      await loadRoutines();
    } catch (err) {
      console.error('Failed to toggle routine:', err);
    }
  };

  const handleTrigger = async (id: string) => {
    try {
      await routineApi.triggerRoutine(id);
    } catch (err) {
      console.error('Failed to trigger routine:', err);
    }
  };

  const handleCreate = async () => {
    if (!newName.trim() || !newDesc.trim()) return;
    try {
      let trigger;
      if (newTrigger === 'manual') trigger = { Manual: null };
      else if (newTrigger === 'time') trigger = { Time: newCron || '0 9 * * *' };
      else trigger = { Event: 'default_event' };

      await routineApi.createRoutine(newName, newDesc, trigger, []);
      setNewName('');
      setNewDesc('');
      setNewTrigger('manual');
      setNewCron('');
      setShowCreate(false);
      await loadRoutines();
    } catch (err) {
      console.error('Failed to create routine:', err);
    }
  };

  const getTriggerType = (r: Routine): string => {
    if (typeof r.trigger === 'string') return r.trigger;
    if (typeof r.trigger === 'object' && r.trigger) {
      const keys = Object.keys(r.trigger);
      return keys[0]?.toLowerCase() || 'manual';
    }
    return 'manual';
  };

  const NAV_ITEMS: { key: FilterKey; label: string; icon: React.ElementType }[] = [
    { key: 'all', label: '全部任务', icon: List },
    { key: 'enabled', label: '已启用', icon: CirclePlay },
    { key: 'disabled', label: '已禁用', icon: CirclePause },
  ];

  const filterLabel = NAV_ITEMS.find((n) => n.key === filter)?.label || '全部任务';

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="flex h-[680px] max-h-[85vh] w-[960px] max-w-[95vw] flex-row gap-0 overflow-hidden rounded-2xl border-border p-0 sm:max-w-[960px]">
        <DialogTitle className="sr-only">定时任务</DialogTitle>
        <DialogDescription className="sr-only">管理定时任务</DialogDescription>

        {/* Left Nav - 200px */}
        <nav className="flex w-[200px] shrink-0 flex-col border-r border-border bg-[#FAFAF8] p-3 pt-6 dark:bg-secondary/50">
          <p className="mb-2 px-3 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
            定时任务
          </p>
          <div className="flex flex-col gap-0.5">
            {NAV_ITEMS.map(({ key, label, icon: Icon }) => (
              <button
                key={key}
                onClick={() => setFilter(key)}
                className={cn(
                  'flex items-center gap-2 rounded-lg px-3 py-2 text-[13px] font-medium transition-colors',
                  filter === key
                    ? 'bg-primary/10 text-primary font-semibold'
                    : 'text-text-secondary hover:bg-accent',
                )}
              >
                <Icon className="size-[15px]" />
                <span className="flex-1 text-left">{label}</span>
                <span
                  className={cn(
                    'flex h-[18px] min-w-[18px] items-center justify-center rounded-full px-1.5 text-[11px] font-semibold',
                    filter === key
                      ? 'bg-primary text-primary-foreground'
                      : 'bg-border text-text-secondary',
                  )}
                >
                  {counts[key]}
                </span>
              </button>
            ))}
          </div>
        </nav>

        {/* Right Content */}
        <div className="flex flex-1 flex-col overflow-hidden">
          {/* Header */}
          <div className="flex h-[60px] items-center justify-between border-b border-border px-6 pr-14">
            <div className="flex items-center gap-2">
              <span className="text-[15px] font-bold text-foreground">{filterLabel}</span>
              <span className="text-[13px] text-muted-foreground">
                {filtered.length} 个定时任务
              </span>
            </div>
            <div className="flex items-center gap-2">
              <Button
                size="sm"
                className="h-8 gap-1.5 rounded-lg text-[13px]"
                onClick={() => setShowCreate(true)}
              >
                <Plus className="size-3.5" />
                新建定时任务
              </Button>
            </div>
          </div>

          {/* Card List */}
          <ScrollArea className="flex-1">
            <div className="space-y-2 p-5">
              {loading && filtered.length === 0 && (
                <p className="py-12 text-center text-sm text-muted-foreground">加载中...</p>
              )}
              {!loading && filtered.length === 0 && (
                <p className="py-12 text-center text-sm text-muted-foreground">暂无定时任务</p>
              )}
              {filtered.map((routine) => {
                const triggerType = getTriggerType(routine);
                const tagColor = TAG_COLORS[triggerType] || TAG_COLORS.manual;
                return (
                  <div
                    key={routine.id}
                    className="rounded-[10px] border border-border bg-card p-4 transition-colors hover:border-primary/20"
                  >
                    {/* Top row: name + tag | toggle + run */}
                    <div className="flex items-center justify-between">
                      <div className="flex items-center gap-2">
                        <span className="text-sm font-semibold text-foreground">
                          {routine.name}
                        </span>
                        <span
                          className={cn(
                            'rounded-full px-2 py-0.5 text-[11px] font-medium',
                            tagColor.bg,
                            tagColor.text,
                          )}
                        >
                          {TRIGGER_LABELS[triggerType] || triggerType}
                        </span>
                      </div>
                      <div className="flex items-center gap-2">
                        <Switch
                          checked={routine.status === 'active'}
                          onCheckedChange={() => handleToggle(routine.id, routine.status)}
                        />
                        <button
                          onClick={() => handleTrigger(routine.id)}
                          className="flex h-7 items-center gap-1 rounded-md bg-secondary px-2.5 text-[12px] font-medium text-text-secondary transition-colors hover:bg-accent"
                        >
                          <Play className="size-3" />
                          运行
                        </button>
                      </div>
                    </div>

                    {/* Description */}
                    <p className="mt-2 text-xs text-muted-foreground">
                      {routine.description}
                    </p>

                    {/* Meta row */}
                    <div className="mt-3 flex items-center gap-4">
                      <span className="flex items-center gap-1 text-[11px] text-muted-foreground">
                        <Clock3 className="size-3 text-text-tertiary" />
                        {TRIGGER_LABELS[triggerType] || triggerType}
                        {routine.triggerValue && ` ${routine.triggerValue}`}
                      </span>
                      <span className="flex items-center gap-1 text-[11px] text-muted-foreground">
                        <CircleCheck className="size-3 text-primary" />
                        上次执行：-
                      </span>
                      <span className="flex items-center gap-1 text-[11px] text-muted-foreground">
                        <Hash className="size-3 text-text-tertiary" />
                        已执行 0 次
                      </span>
                    </div>
                  </div>
                );
              })}
            </div>
          </ScrollArea>
        </div>

        {/* Create Dialog (inline overlay) */}
        {showCreate && (
          <div className="absolute inset-0 z-10 flex items-center justify-center bg-black/30">
            <div className="w-[400px] rounded-2xl border border-border bg-card p-6 shadow-xl">
              <h3 className="mb-4 text-base font-bold text-foreground">新建定时任务</h3>
              <div className="space-y-3">
                <div>
                  <label className="mb-1 block text-xs font-medium text-foreground">名称</label>
                  <input
                    value={newName}
                    onChange={(e) => setNewName(e.target.value)}
                    placeholder="输入任务名称"
                    className="w-full rounded-lg border border-border bg-secondary px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:border-primary focus:outline-none"
                  />
                </div>
                <div>
                  <label className="mb-1 block text-xs font-medium text-foreground">描述</label>
                  <textarea
                    value={newDesc}
                    onChange={(e) => setNewDesc(e.target.value)}
                    placeholder="输入任务描述"
                    rows={2}
                    className="w-full resize-none rounded-lg border border-border bg-secondary px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:border-primary focus:outline-none"
                  />
                </div>
                <div>
                  <label className="mb-1 block text-xs font-medium text-foreground">触发方式</label>
                  <select
                    value={newTrigger}
                    onChange={(e) => setNewTrigger(e.target.value as 'manual' | 'time' | 'event')}
                    className="w-full rounded-lg border border-border bg-secondary px-3 py-2 text-sm text-foreground focus:border-primary focus:outline-none"
                  >
                    <option value="manual">手动</option>
                    <option value="time">时间（Cron）</option>
                    <option value="event">事件</option>
                  </select>
                </div>
                {newTrigger === 'time' && (
                  <div>
                    <label className="mb-1 block text-xs font-medium text-foreground">Cron 表达式</label>
                    <input
                      value={newCron}
                      onChange={(e) => setNewCron(e.target.value)}
                      placeholder="例如: 0 9 * * *"
                      className="w-full rounded-lg border border-border bg-secondary px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:border-primary focus:outline-none"
                    />
                  </div>
                )}
              </div>
              <div className="mt-5 flex justify-end gap-2">
                <Button variant="outline" size="sm" onClick={() => setShowCreate(false)}>
                  取消
                </Button>
                <Button size="sm" onClick={handleCreate} disabled={!newName.trim()}>
                  创建
                </Button>
              </div>
            </div>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}
