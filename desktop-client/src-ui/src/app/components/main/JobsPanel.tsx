/**
 * JobsPanel - 任务面板（右侧抽屉）
 *
 * 设计稿：380px 右侧面板，包含统计行 + 任务列表。
 * 使用 shadcn Sheet 组件。
 */

import { useState, useEffect } from 'react';
import {
  Briefcase,
  CheckCircle,
  XCircle,
  Loader2,
} from 'lucide-react';
import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
  SheetDescription,
} from '../ui/sheet';
import { ScrollArea } from '../ui/scroll-area';
import { cn } from '../ui/utils';
import { jobApi, type JobInfo } from '../../utils/tauri';

interface JobsPanelProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

type StatusKey = 'running' | 'completed' | 'failed';

const STATUS_CONFIG: Record<StatusKey, { label: string; color: string; bgColor: string; icon: React.ElementType }> = {
  running: { label: '运行中', color: 'text-amber-600', bgColor: 'bg-amber-50', icon: Loader2 },
  completed: { label: '已完成', color: 'text-emerald-600', bgColor: 'bg-emerald-50', icon: CheckCircle },
  failed: { label: '失败', color: 'text-red-600', bgColor: 'bg-red-50', icon: XCircle },
};

export function JobsPanel({ open, onOpenChange }: JobsPanelProps) {
  const [jobs, setJobs] = useState<JobInfo[]>([]);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (open) fetchJobs();
  }, [open]);

  const fetchJobs = async () => {
    try {
      setLoading(true);
      const list = await jobApi.getJobs();
      setJobs(list);
    } catch (err) {
      console.error('Failed to fetch jobs:', err);
    } finally {
      setLoading(false);
    }
  };

  const counts = {
    running: jobs.filter((j) => j.status === 'in_progress' || j.status === 'pending').length,
    completed: jobs.filter((j) => j.status === 'completed').length,
    failed: jobs.filter((j) => j.status === 'failed').length,
  };

  const mapStatus = (status: string): StatusKey => {
    if (status === 'completed') return 'completed';
    if (status === 'failed') return 'failed';
    return 'running';
  };

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent side="right" className="w-[380px] p-0 sm:max-w-[380px]">
        {/* Header */}
        <SheetHeader className="border-b border-border px-5 py-4">
          <div className="flex items-center gap-2">
            <Briefcase className="size-4 text-foreground" />
            <SheetTitle className="text-base">任务列表</SheetTitle>
          </div>
          <SheetDescription className="sr-only">查看和管理任务</SheetDescription>
        </SheetHeader>

        {/* Stats Row */}
        <div className="grid grid-cols-3 gap-3 border-b border-border px-5 py-3">
          {(Object.entries(STATUS_CONFIG) as [StatusKey, typeof STATUS_CONFIG[StatusKey]][]).map(
            ([key, cfg]) => {
              const Icon = cfg.icon;
              return (
                <div
                  key={key}
                  className={cn('flex flex-col items-center rounded-lg py-2', cfg.bgColor)}
                >
                  <Icon className={cn('size-4 mb-1', cfg.color, key === 'running' && 'animate-spin')} />
                  <span className="text-lg font-bold text-foreground">{counts[key]}</span>
                  <span className="text-[11px] text-muted-foreground">{cfg.label}</span>
                </div>
              );
            },
          )}
        </div>

        {/* Job List */}
        <ScrollArea className="flex-1">
          <div className="space-y-2 p-4">
            {jobs.length === 0 && !loading && (
              <div className="py-12 text-center text-sm text-muted-foreground">
                <Briefcase className="mx-auto mb-3 size-8 opacity-40" />
                暂无任务
              </div>
            )}
            {loading && jobs.length === 0 && (
              <div className="py-12 text-center text-sm text-muted-foreground">
                <Loader2 className="mx-auto mb-3 size-8 animate-spin opacity-40" />
                加载中...
              </div>
            )}
            {jobs.map((job) => {
              const sk = mapStatus(job.status);
              const cfg = STATUS_CONFIG[sk];
              const Icon = cfg.icon;
              return (
                <div
                  key={job.id}
                  className="rounded-xl border border-border bg-card p-3 transition-colors hover:border-primary/20"
                >
                  <div className="flex items-start justify-between">
                    <div className="flex-1 pr-2">
                      <p className="text-sm font-medium text-foreground line-clamp-1">
                        {job.title || job.id}
                      </p>
                      <p className="mt-0.5 text-xs text-muted-foreground">
                        {new Date(job.created_at).toLocaleString('zh-CN')}
                      </p>
                    </div>
                    <span
                      className={cn(
                        'inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[11px] font-medium',
                        cfg.bgColor,
                        cfg.color,
                      )}
                    >
                      <Icon className={cn('size-3', sk === 'running' && 'animate-spin')} />
                      {cfg.label}
                    </span>
                  </div>
                </div>
              );
            })}
          </div>
        </ScrollArea>
      </SheetContent>
    </Sheet>
  );
}
