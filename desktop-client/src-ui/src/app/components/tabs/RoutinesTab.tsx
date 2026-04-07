/**
 * RoutinesTab - 定时任务管理
 *
 * 设计稿：960x680 模态框，左侧 200px 导航（全部/已启用/已禁用）+ 右侧任务卡片列表。
 * 每张卡片包含：名称 + 标签 + 开关 + 运行按钮 + 描述 + 元信息（触发方式/上次执行/执行次数）。
 */

import { useState, useEffect, useCallback } from 'react';
import {
  Plus,
  List,
  CirclePlay,
  CirclePause,
  Clock3,
  CircleCheck,
  Hash,
  Play,
  Trash2,
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
import { useEngineReady } from '../../hooks/useEngineReady';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '../ui/select';
import {
  CronSchedulePicker,
  DEFAULT_CRON_SCHEDULE,
  toCronExpression,
  type CronSchedule,
} from '../ui/CronSchedulePicker';

type FilterKey = 'all' | 'enabled' | 'disabled';

interface RoutinesTabProps {
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
  onRoutineFired?: (threadId: string, prompt: string) => void;
}

const TRIGGER_LABELS: Record<string, string> = {
  manual: '手动触发',
  time: '时间触发',
  event: '事件触发',
};

const TAG_COLORS: Record<string, { bg: string; text: string }> = {
  time: { bg: 'bg-blue-50', text: 'text-blue-600' },
  event: { bg: 'bg-amber-50', text: 'text-amber-600' },
  manual: { bg: 'bg-secondary', text: 'text-muted-foreground' },
};

export function RoutinesTab({ open = true, onOpenChange, onRoutineFired }: RoutinesTabProps) {
  const [filter, setFilter] = useState<FilterKey>('all');
  const [routines, setRoutines] = useState<Routine[]>([]);
  const [loading, setLoading] = useState(false);
  const [firingId, setFiringId] = useState<string | null>(null);
  const [firingError, setFiringError] = useState<string | null>(null);
  const [showCreate, setShowCreate] = useState(false);
  const [newName, setNewName] = useState('');
  const [newDesc, setNewDesc] = useState('');
  const [newTrigger, setNewTrigger] = useState<'manual' | 'time' | 'event'>('manual');
  // 执行历史：key 为 routine.id，value 为 { lastRun, runCount }
  const [runStats, setRunStats] = useState<Record<string, { lastRun: string | null; runCount: number }>>({});
  const [newCronSchedule, setNewCronSchedule] = useState<CronSchedule>(DEFAULT_CRON_SCHEDULE);
  const [newPrompt, setNewPrompt] = useState('');
  const [newPattern, setNewPattern] = useState('');
  const [createError, setCreateError] = useState<string | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const { readyKey } = useEngineReady();

  const loadRoutines = useCallback(async () => {
    setLoading(true);
    setLoadError(null);
    try {
      const data = await routineApi.getRoutines();
      setRoutines(data);
      const stats: Record<string, { lastRun: string | null; runCount: number }> = {};
      await Promise.allSettled(
        data.map(async (r) => {
          try {
            const res = await routineExtendedApi.getRoutineRuns(r.id);
            stats[r.id] = {
              lastRun: res.runs[0]?.started_at ?? null,
              runCount: res.runs.length,
            };
          } catch {
            stats[r.id] = { lastRun: null, runCount: 0 };
          }
        }),
      );
      setRunStats(stats);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      console.error('Failed to load routines:', msg);
      setLoadError(msg);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (open) loadRoutines();
  }, [open, loadRoutines, readyKey]);

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
    setFiringId(id);
    setFiringError(null);
    try {
      const result = await routineApi.triggerRoutine(id);
      onOpenChange?.(false);
      onRoutineFired?.(result.thread_id, result.prompt);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      // 并发限制：任务正在运行中
      if (msg.includes('max concurrent')) {
        setFiringError('任务正在运行中，请等待当前执行完成后再触发');
      } else {
        setFiringError(msg);
      }
    } finally {
      setFiringId(null);
    }
  };

  const handleDelete = async (id: string) => {
    try {
      await routineApi.deleteRoutine(id);
      await loadRoutines();
    } catch (err) {
      console.error('Failed to delete routine:', err);
    }
  };

  const handleCreate = async () => {
    if (!newName.trim() || !newDesc.trim() || !newPrompt.trim()) return;
    setCreateError(null);
    try {
      let trigger: Record<string, unknown>;
      if (newTrigger === 'manual') trigger = { type: 'manual' };
      else if (newTrigger === 'time') trigger = { type: 'cron', schedule: toCronExpression(newCronSchedule) };
      else trigger = { type: 'event', pattern: newPattern || '.*', channel: null };

      await routineApi.createRoutine(newName, newDesc, trigger, [], newPrompt);
      setNewName('');
      setNewDesc('');
      setNewTrigger('manual');
      setNewCronSchedule(DEFAULT_CRON_SCHEDULE);
      setNewPrompt('');
      setNewPattern('');
      setShowCreate(false);
      await loadRoutines();
    } catch (err) {
      console.error('Failed to create routine:', err);
      setCreateError(err instanceof Error ? err.message : String(err));
    }
  };

  const getTriggerType = (r: Routine): string => {
    if (typeof r.trigger === 'string') return r.trigger;
    if (typeof r.trigger === 'object' && r.trigger) {
      // 后端返回 { type: "manual" | "cron" | "event" | ... }
      const t = (r.trigger as Record<string, unknown>).type;
      if (typeof t === 'string') return t.toLowerCase();
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
              {!loading && loadError && (
                <div className="rounded-lg border border-destructive/30 bg-destructive/5 px-4 py-3 text-xs text-destructive">
                  加载失败：{loadError}
                </div>
              )}
              {firingError && (
                <div className="rounded-lg border border-amber-200 bg-amber-50 px-4 py-3 text-xs text-amber-700 dark:border-amber-800 dark:bg-amber-950/30 dark:text-amber-400">
                  {firingError}
                </div>
              )}
              {!loading && !loadError && filtered.length === 0 && (
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
                        <Button
                          variant="outline"
                          size="xs"
                          onClick={() => handleTrigger(routine.id)}
                          disabled={firingId === routine.id}
                        >
                          <Play className={cn('size-3', firingId === routine.id && 'animate-pulse')} />
                          {firingId === routine.id ? '运行中' : '立即运行'}
                        </Button>
                        <Button
                          variant="ghost"
                          size="icon-xs"
                          onClick={() => handleDelete(routine.id)}
                          title="删除任务"
                        >
                          <Trash2 className="size-3.5" />
                        </Button>
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
                        {routine.trigger?.schedule && ` ${routine.trigger.schedule}`}
                      </span>
                      <span className="flex items-center gap-1 text-[11px] text-muted-foreground">
                        <CircleCheck className="size-3 text-primary" />
                        上次执行：{runStats[routine.id]?.lastRun
                          ? new Date(runStats[routine.id].lastRun!).toLocaleString('zh-CN', { month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' })
                          : '-'}
                      </span>
                      <span className="flex items-center gap-1 text-[11px] text-muted-foreground">
                        <Hash className="size-3 text-text-tertiary" />
                        已执行 {runStats[routine.id]?.runCount ?? 0} 次
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
              {createError && (
                <div className="mb-3 rounded-lg border border-destructive/30 bg-destructive/5 px-3 py-2 text-xs text-destructive">
                  {createError}
                </div>
              )}
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
                  <Select
                    value={newTrigger}
                    onValueChange={(v) => setNewTrigger(v as 'manual' | 'time' | 'event')}
                  >
                    <SelectTrigger className="w-full">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="manual">手动</SelectItem>
                      <SelectItem value="time">时间（Cron）</SelectItem>
                      <SelectItem value="event">事件</SelectItem>
                    </SelectContent>
                  </Select>
                </div>
                {newTrigger === 'time' && (
                  <div>
                    <label className="mb-1 block text-xs font-medium text-foreground">执行频率</label>
                    <CronSchedulePicker
                      value={newCronSchedule}
                      onChange={setNewCronSchedule}
                    />
                  </div>
                )}
                {newTrigger === 'event' && (
                  <div>
                    <label className="mb-1 block text-xs font-medium text-foreground">触发正则（匹配消息内容）</label>
                    <input
                      value={newPattern}
                      onChange={(e) => setNewPattern(e.target.value)}
                      placeholder="例如: .*报告.*（留空则匹配所有消息）"
                      className="w-full rounded-lg border border-border bg-secondary px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:border-primary focus:outline-none"
                    />
                  </div>
                )}
                <div>
                  <label className="mb-1 block text-xs font-medium text-foreground">
                    执行提示词 <span className="text-destructive">*</span>
                  </label>
                  <textarea
                    value={newPrompt}
                    onChange={(e) => setNewPrompt(e.target.value)}
                    placeholder="任务触发时发给 AI 的指令，例如：总结今日工作进展并发送通知"
                    rows={3}
                    className="w-full resize-none rounded-lg border border-border bg-secondary px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:border-primary focus:outline-none"
                  />
                </div>
              </div>
              <div className="mt-5 flex justify-end gap-2">
                <Button variant="outline" size="sm" onClick={() => setShowCreate(false)}>
                  取消
                </Button>
                <Button size="sm" onClick={handleCreate} disabled={!newName.trim() || !newDesc.trim() || !newPrompt.trim()}>
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
