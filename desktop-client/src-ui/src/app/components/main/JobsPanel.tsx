/**
 * JobsPanel - 任务面板（右侧抽屉）
 *
 * 设计稿：380px 右侧面板，包含统计行 + 任务列表。
 * 点击任务卡片后加载事件详情，支持跳转到关联对话。
 * 使用 shadcn Sheet 组件。
 */

import { useState, useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import {
  Briefcase,
  CheckCircle,
  XCircle,
  Loader2,
  ArrowLeft,
  MessageSquare,
  History,
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
import { jobApi, type JobInfo, type JobDetail } from '../../utils/tauri';
import { useEngineReady } from '../../hooks/useEngineReady';

interface JobsPanelProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** 在当前对话中发起“查看任务结果”提问 */
  onAskJobResult?: (question: string) => void;
  /** 点击任务对话入口时回调 */
  onJobClick?: (conversationId: string) => void;
}

type StatusKey = 'running' | 'completed' | 'failed' | 'cancelled';

const STATUS_CONFIG: Record<StatusKey, { label: string; color: string; bgColor: string; icon: React.ElementType }> = {
  running: { label: '运行中', color: 'text-amber-600', bgColor: 'bg-amber-50', icon: Loader2 },
  completed: { label: '已完成', color: 'text-emerald-600', bgColor: 'bg-emerald-50', icon: CheckCircle },
  failed: { label: '失败', color: 'text-red-600', bgColor: 'bg-red-50', icon: XCircle },
  cancelled: { label: '已取消', color: 'text-slate-600', bgColor: 'bg-slate-100', icon: XCircle },
};

const EVENT_TYPE_LABELS: Record<string, string> = {
  started: '开始执行',
  completed: '执行完成',
  failed: '执行失败',
  step: '执行步骤',
  tool_call: '工具调用',
  tool_result: '工具结果',
  message: '消息',
  error: '错误',
  progress: '进度',
};

export function JobsPanel({ open, onOpenChange, onAskJobResult, onJobClick }: JobsPanelProps) {
  const [jobs, setJobs] = useState<JobInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [selectedJob, setSelectedJob] = useState<JobInfo | null>(null);
  const [jobDetail, setJobDetail] = useState<JobDetail | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const { readyKey } = useEngineReady();

  useEffect(() => {
    if (open) {
      fetchJobs();
    } else {
      // 关闭抽屉时重置详情视图
      setSelectedJob(null);
      setJobDetail(null);
    }
  }, [open, readyKey]);

  const fetchJobs = async () => {
    try {
      setLoading(true);
      const list = await jobApi.getJobs();
      const sorted = [...list].sort((a, b) => {
        const ta = Date.parse(a.created_at);
        const tb = Date.parse(b.created_at);
        if (Number.isFinite(ta) && Number.isFinite(tb) && ta !== tb) {
          return tb - ta;
        }
        return b.id.localeCompare(a.id);
      });
      setJobs(sorted);
    } catch (err) {
      console.error('Failed to fetch jobs:', err);
    } finally {
      setLoading(false);
    }
  };

  // 抽屉打开时监听任务事件，自动刷新列表，避免“刚触发看不到任务”的竞态。
  useEffect(() => {
    if (!open) return;

    let timer: ReturnType<typeof setTimeout> | null = null;
    const scheduleRefresh = () => {
      if (timer) return;
      timer = setTimeout(() => {
        timer = null;
        void fetchJobs();
      }, 300);
    };

    const JOB_EVENT_TYPES = new Set(['job_status', 'job_started', 'job_updated', 'job_completed', 'job_failed']);
    const unlistenPromise = listen<{ type?: string }>('chat-event', (event) => {
      if (JOB_EVENT_TYPES.has(event.payload?.type ?? '')) {
        scheduleRefresh();
      }
    });

    return () => {
      if (timer) clearTimeout(timer);
      unlistenPromise.then((fn) => fn()).catch(() => {});
    };
  }, [open]);

  // 打开抽屉时兜底轮询，避免事件丢失导致新任务不可见。
  useEffect(() => {
    if (!open) return;
    const interval = setInterval(() => {
      void fetchJobs();
    }, 1500);
    return () => clearInterval(interval);
  }, [open]);

  const handleJobClick = async (job: JobInfo) => {
    setSelectedJob(job);
    setDetailLoading(true);
    try {
      const detail = await jobApi.getJobDetail(job.id);
      setJobDetail(detail);
    } catch (err) {
      console.error('Failed to fetch job detail:', err);
      setJobDetail(null);
    } finally {
      setDetailLoading(false);
    }
  };

  const handleBack = () => {
    setSelectedJob(null);
    setJobDetail(null);
  };

  const handleGoToConversation = () => {
    if (!jobDetail?.conversation_id) return;
    onJobClick?.(jobDetail.conversation_id);
  };

  const handleAskJobResult = () => {
    if (!jobDetail) return;
    const title = jobDetail.title || selectedJob?.title || selectedJob?.id || jobDetail.id;
    const summaryHint = jobDetail.description?.trim() ? `任务描述：${jobDetail.description.trim()}\n` : '';
    const question = [
      `请基于任务执行记录总结这个任务的运行结果：`,
      `任务ID：${jobDetail.id}`,
      `任务标题：${title}`,
      `当前状态：${jobDetail.status}`,
      summaryHint,
      '请输出：1) 执行结论 2) 关键步骤 3) 若失败请给出修复建议。',
    ]
      .filter(Boolean)
      .join('\n');
    onAskJobResult?.(question);
  };

  const counts = {
    running: jobs.filter((j) => j.status === 'in_progress' || j.status === 'pending').length,
    completed: jobs.filter((j) => j.status === 'completed').length,
    failed: jobs.filter((j) => j.status === 'failed' || j.status === 'cancelled').length,
  };

  const mapStatus = (status: string): StatusKey => {
    if (status === 'completed') return 'completed';
    if (status === 'failed') return 'failed';
    if (status === 'cancelled') return 'cancelled';
    return 'running';
  };

  // ── 任务详情视图 ──────────────────────────────────────
  const renderDetail = () => {
    if (!selectedJob) return null;
    const sk = mapStatus(selectedJob.status);
    const cfg = STATUS_CONFIG[sk];
    const Icon = cfg.icon;

    return (
      <>
        {/* Detail Header */}
        <div className="border-b border-border px-5 py-3">
          <div className="flex items-center gap-2">
            <button
              onClick={handleBack}
              className="rounded-md p-1 text-muted-foreground hover:bg-accent hover:text-foreground"
            >
              <ArrowLeft className="size-4" />
            </button>
            <div className="flex-1 min-w-0">
              <p className="text-sm font-medium text-foreground truncate">
                {selectedJob.title || selectedJob.id}
              </p>
              <div className="flex items-center gap-2 mt-0.5">
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
                <span className="text-xs text-muted-foreground">
                  {new Date(selectedJob.created_at).toLocaleString('zh-CN')}
                </span>
              </div>
            </div>
          </div>
          {jobDetail?.conversation_id && (
            <button
              onClick={handleGoToConversation}
              className="mt-2 flex w-full items-center justify-center gap-1.5 rounded-lg border border-border bg-background px-3 py-1.5 text-xs font-medium text-foreground transition-colors hover:bg-accent"
            >
              <MessageSquare className="size-3.5" />
              打开任务对话
            </button>
          )}
        </div>

        {/* Detail Body */}
        <ScrollArea className="flex-1">
          {detailLoading && (
            <div className="py-12 text-center text-sm text-muted-foreground">
              <Loader2 className="mx-auto mb-2 size-6 animate-spin opacity-40" />
              加载详情中...
            </div>
          )}
          {!detailLoading && jobDetail && (
            <div className="p-4 space-y-4">
              {/* 任务描述 */}
              <div className="space-y-2">
                {jobDetail.description && (
                  <>
                    <p className="mb-1 text-xs font-medium text-muted-foreground">任务描述</p>
                    <p className="text-sm text-foreground">{jobDetail.description}</p>
                  </>
                )}
                <button
                  type="button"
                  onClick={handleAskJobResult}
                  className="inline-flex items-center gap-1.5 rounded-lg border border-border bg-background px-3 py-1.5 text-xs font-medium text-foreground transition-colors hover:bg-accent"
                >
                  <MessageSquare className="size-3.5" />
                  查看任务结果
                </button>
              </div>

              {/* 失败原因 */}
              {jobDetail.failure_reason && (
                <div className="rounded-lg border border-red-200 bg-red-50 p-3">
                  <p className="mb-1 text-xs font-medium text-red-600">失败原因</p>
                  <p className="text-xs text-red-700 whitespace-pre-wrap break-all">{jobDetail.failure_reason}</p>
                </div>
              )}

              {/* Token 用量 */}
              {!!jobDetail.total_tokens_used && (
                <div className="flex items-center gap-2">
                  <span className="text-xs text-muted-foreground">Token 消耗</span>
                  <span className="rounded-full bg-muted px-2 py-0.5 text-xs font-medium text-foreground">
                    {jobDetail.total_tokens_used.toLocaleString()}
                  </span>
                </div>
              )}

              {/* 事件历史 */}
              <div>
                <div className="mb-2 flex items-center gap-1.5 text-xs font-medium text-muted-foreground">
                  <History className="size-3.5" />
                  事件历史 ({jobDetail.events.length})
                </div>
                {jobDetail.events.length === 0 ? (
                  <div className="py-4 text-center text-sm text-muted-foreground">暂无事件记录</div>
                ) : (
                  <div className="space-y-2">
                    {jobDetail.events.map((evt) => (
                      <div
                        key={evt.id}
                        className="rounded-lg border border-border bg-card p-2.5 text-xs"
                      >
                        <div className="flex items-center justify-between">
                          <span className="font-medium text-foreground">
                            {EVENT_TYPE_LABELS[evt.event_type] || evt.event_type}
                          </span>
                          <span className="text-muted-foreground">
                            {new Date(evt.created_at).toLocaleTimeString('zh-CN')}
                          </span>
                        </div>
                        {evt.data && (
                          <pre className="mt-1.5 max-h-32 overflow-auto whitespace-pre-wrap break-all rounded bg-muted/50 p-2 text-[11px] text-muted-foreground">
                            {typeof evt.data === 'string' ? evt.data : JSON.stringify(evt.data, null, 2)}
                          </pre>
                        )}
                      </div>
                    ))}
                  </div>
                )}
              </div>
            </div>
          )}
        </ScrollArea>
      </>
    );
  };

  // ── 任务列表视图 ──────────────────────────────────────
  const renderList = () => (
    <>
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
                onClick={() => handleJobClick(job)}
                className="cursor-pointer rounded-xl border border-border bg-card p-3 transition-colors hover:border-primary/20 hover:bg-accent/30"
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
    </>
  );

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

        {selectedJob ? renderDetail() : renderList()}
      </SheetContent>
    </Sheet>
  );
}
