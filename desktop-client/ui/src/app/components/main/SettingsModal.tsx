/**
 * SettingsModal - 设置弹窗
 *
 * 设计稿：900x700 模态框，左侧 200px 导航 + 右侧内容区。
 * 导航项：通用设置 / 用量统计 / 诊断 / 技能管理 / 扩展 / 记忆 / 关于我们
 */

import { useEffect, useState } from 'react';
import {
  Settings,
  BarChart3,
  Puzzle,
  Brain,
  Info,
  LogOut,
  Shield,
  Moon,
  User,
  Zap,
  Activity,
  AlertCircle,
  CheckCircle2,
  RefreshCw,
  Server,
} from 'lucide-react';
import {
  Dialog,
  DialogContent,
  DialogTitle,
  DialogDescription,
} from '../ui/dialog';
import { Switch } from '../ui/switch';
import { ScrollArea } from '../ui/scroll-area';
import { cn } from '../ui/utils';
import { SkillsTab } from '../tabs/SkillsTab';
import { ExtensionsTab } from '../tabs/ExtensionsTab';
import { MemoryTab } from '../tabs/MemoryTab';
import { AboutTab } from '../tabs/AboutTab';
import { appServerApi, type AppServerStatusReport } from '../../utils/tauri';

type SettingsNav =
  | 'general'
  | 'usage'
  | 'diagnostics'
  | 'skills'
  | 'extensions'
  | 'memory'
  | 'about';

const NAV_ITEMS: { key: SettingsNav; label: string; icon: React.ElementType }[] = [
  { key: 'general', label: '通用设置', icon: Settings },
  { key: 'usage', label: '用量统计', icon: BarChart3 },
  { key: 'diagnostics', label: '诊断', icon: Activity },
  { key: 'skills', label: '技能管理', icon: Zap },
  { key: 'extensions', label: '扩展', icon: Puzzle },
  { key: 'memory', label: '记忆', icon: Brain },
  { key: 'about', label: '关于我们', icon: Info },
];

interface SettingsModalProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function SettingsModal({ open, onOpenChange }: SettingsModalProps) {
  const [activeNav, setActiveNav] = useState<SettingsNav>('general');
  const [aiSafety, setAiSafety] = useState(true);
  const [antiSleep, setAntiSleep] = useState(false);

  const handleLogout = () => {
    if (confirm('确定要退出登录吗？')) {
      window.location.reload();
    }
  };

  const renderContent = () => {
    switch (activeNav) {
      case 'general':
        return <GeneralSettings aiSafety={aiSafety} onAiSafetyChange={setAiSafety} antiSleep={antiSleep} onAntiSleepChange={setAntiSleep} onLogout={handleLogout} />;
      case 'usage':
        return <UsageStats />;
      case 'diagnostics':
        return <DiagnosticsSettings />;
      case 'skills':
        return <SkillsTab />;
      case 'extensions':
        return <ExtensionsTab />;
      case 'memory':
        return <MemoryTab />;
      case 'about':
        return <AboutTab />;
      default:
        return null;
    }
  };

  const currentNav = NAV_ITEMS.find((n) => n.key === activeNav);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="flex h-[700px] max-h-[85vh] w-[900px] max-w-[95vw] flex-row gap-0 overflow-hidden rounded-2xl border-border p-0 sm:max-w-[900px]">
        <DialogTitle className="sr-only">设置</DialogTitle>
        <DialogDescription className="sr-only">应用设置面板</DialogDescription>

        {/* Left Nav */}
        <nav className="flex w-[200px] shrink-0 flex-col border-r border-border bg-secondary/50 p-3">
          <div className="mb-4 px-3 pt-2">
            <h2 className="text-sm font-bold text-foreground">设置</h2>
          </div>
          <div className="flex flex-1 flex-col gap-0.5">
            {NAV_ITEMS.map(({ key, label, icon: Icon }) => (
              <button
                key={key}
                onClick={() => setActiveNav(key)}
                className={cn(
                  'flex items-center gap-2.5 rounded-lg px-3 py-2 text-[13px] font-medium transition-colors',
                  activeNav === key
                    ? 'bg-primary/10 text-primary'
                    : 'text-text-secondary hover:bg-accent hover:text-foreground',
                )}
              >
                <Icon className="size-4" />
                {label}
              </button>
            ))}
          </div>
        </nav>

        {/* Right Content */}
        <div className="flex flex-1 flex-col overflow-hidden">
          {/* Header */}
          <div className="flex items-center justify-between border-b border-border px-6 pr-14 py-4">
            <div>
              <h3 className="text-base font-bold text-foreground">
                {currentNav?.label}
              </h3>
              <p className="mt-0.5 text-xs text-muted-foreground">
                {getSubtitle(activeNav)}
              </p>
            </div>
          </div>

          {/* Body */}
          <ScrollArea className="flex-1">
            <div className="p-6">{renderContent()}</div>
          </ScrollArea>
        </div>
      </DialogContent>
    </Dialog>
  );
}


function getSubtitle(nav: SettingsNav): string {
  switch (nav) {
    case 'general': return '管理账户、安全和系统偏好';
    case 'usage': return '查看资源使用情况和统计数据';
    case 'diagnostics': return '查看本地运行组件状态';
    case 'skills': return '为您的智能体提供预装且可重复的最佳实践与工具';
    case 'extensions': return '浏览和管理扩展';
    case 'memory': return '查看和管理记忆数据';
    case 'about': return '关于 X-Claw 桌面客户端';
    default: return '';
  }
}

/* ── 通用设置 ── */
function GeneralSettings({
  aiSafety,
  onAiSafetyChange,
  antiSleep,
  onAntiSleepChange,
  onLogout,
}: {
  aiSafety: boolean;
  onAiSafetyChange: (v: boolean) => void;
  antiSleep: boolean;
  onAntiSleepChange: (v: boolean) => void;
  onLogout: () => void;
}) {
  return (
    <div className="space-y-4">
      {/* Account Card */}
      <div className="flex items-center gap-4 rounded-xl border border-border bg-card p-4">
        <div className="flex size-12 items-center justify-center rounded-full bg-primary text-lg font-bold text-primary-foreground">
          <User className="size-5" />
        </div>
        <div>
          <p className="text-sm font-semibold text-foreground">用户</p>
          <p className="text-xs text-muted-foreground">user@example.com</p>
        </div>
      </div>

      {/* Security Card */}
      <div className="flex items-center justify-between rounded-xl border border-border bg-card p-4">
        <div className="flex items-center gap-3">
          <Shield className="size-5 text-primary" />
          <div>
            <p className="text-sm font-semibold text-foreground">AI 安全防护</p>
            <p className="text-xs text-muted-foreground">
              启用 DLP 敏感信息检测和过滤
            </p>
          </div>
        </div>
        <Switch checked={aiSafety} onCheckedChange={onAiSafetyChange} />
      </div>

      {/* Anti-Sleep Card */}
      <div className="flex items-center justify-between rounded-xl border border-border bg-card p-4">
        <div className="flex items-center gap-3">
          <Moon className="size-5 text-primary" />
          <div>
            <p className="text-sm font-semibold text-foreground">防休眠</p>
            <p className="text-xs text-muted-foreground">
              防止系统在任务运行时进入休眠
            </p>
          </div>
        </div>
        <Switch checked={antiSleep} onCheckedChange={onAntiSleepChange} />
      </div>

      {/* Logout */}
      <button
        onClick={onLogout}
        className="flex w-full items-center justify-center gap-2 rounded-xl border border-destructive/30 bg-destructive/5 px-4 py-3 text-sm font-medium text-destructive transition-colors hover:bg-destructive/10"
      >
        <LogOut className="size-4" />
        退出登录
      </button>
    </div>
  );
}

/* ── 用量统计 ── */
function UsageStats() {
  return (
    <div className="space-y-4">
      <div className="grid grid-cols-2 gap-4">
        <StatCard label="本月对话数" value="128" />
        <StatCard label="Token 使用量" value="45.2K" />
        <StatCard label="任务完成数" value="34" />
        <StatCard label="活跃天数" value="22" />
      </div>
    </div>
  );
}

function StatCard({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-xl border border-border bg-card p-4">
      <p className="text-xs text-muted-foreground">{label}</p>
      <p className="mt-1 text-2xl font-bold text-foreground">{value}</p>
    </div>
  );
}

/* ── 诊断 ── */
function DiagnosticsSettings(): JSX.Element {
  const [status, setStatus] = useState<AppServerStatusReport | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function loadStatus(runSmoke: boolean): Promise<void> {
    setLoading(true);
    setError(null);
    try {
      const report = await appServerApi.getStatus(runSmoke);
      setStatus(report);
      setError(report.error ?? null);
    } catch (err) {
      setStatus(null);
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void loadStatus(false);
  }, []);

  return (
    <div className="space-y-4">
      <div className="rounded-xl border border-border bg-card p-4">
        <div className="flex items-center justify-between gap-3">
          <div className="flex min-w-0 items-center gap-3">
            <Server className="size-5 shrink-0 text-primary" />
            <div className="min-w-0">
              <p className="text-sm font-semibold text-foreground">dasclaw app-server</p>
              <p className="truncate text-xs text-muted-foreground">
                {status?.binary ?? 'dasclaw-app-server'}
              </p>
            </div>
          </div>
          <button
            type="button"
            onClick={() => void loadStatus(true)}
            disabled={loading}
            className="inline-flex shrink-0 items-center gap-2 rounded-lg border border-border px-3 py-2 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:cursor-not-allowed disabled:opacity-60"
          >
            <RefreshCw className={`size-3.5 ${loading ? 'animate-spin' : ''}`} />
            刷新
          </button>
        </div>

        <div className="mt-4 grid gap-3 sm:grid-cols-2">
          <DiagnosticMetric
            label="启动 smoke"
            value={status?.startupSmokeEnabled ? '已启用' : '未启用'}
            tone={status?.startupSmokeEnabled ? 'ok' : 'muted'}
          />
          <DiagnosticMetric
            label="本次 smoke"
            value={status?.smokeRun ? '已运行' : '未运行'}
            tone={status?.smokeRun ? 'ok' : 'muted'}
          />
          <DiagnosticMetric
            label="Supervisor"
            value={status?.supervisorEnabled ? '已启用' : '未启用'}
            tone={status?.supervisorEnabled ? 'ok' : 'muted'}
          />
          <DiagnosticMetric
            label="Supervisor 状态"
            value={status?.supervisor?.state ?? 'stopped'}
            tone={supervisorTone(status?.supervisor?.state)}
          />
          <DiagnosticMetric
            label="Session"
            value={status?.smoke?.sessionStatus ?? '待检查'}
            tone={status?.smoke?.sessionStatus === 'implemented' ? 'ok' : 'muted'}
          />
          <DiagnosticMetric
            label="Runtime"
            value={status?.smoke?.runtimeHealthStatus ?? '待检查'}
            tone={runtimeTone(status?.smoke?.runtimeHealthStatus)}
          />
        </div>

        {status?.smoke ? (
          <div className="mt-3 rounded-lg bg-secondary/50 px-3 py-2 text-xs text-muted-foreground">
            通知 {status.smoke.notificationCount} 条，shutdown: {status.smoke.shutdownState}
          </div>
        ) : null}

        {status?.supervisor ? (
          <div className="mt-3 rounded-lg bg-secondary/50 px-3 py-2 text-xs text-muted-foreground">
            Supervisor 通知 {status.supervisor.notificationCount} 条，restart:{' '}
            {status.supervisor.restartCount}
          </div>
        ) : null}

        {error ? (
          <div className="mt-3 flex gap-2 rounded-lg border border-destructive/30 bg-destructive/5 px-3 py-2 text-xs text-destructive">
            <AlertCircle className="mt-0.5 size-3.5 shrink-0" />
            <span className="break-words">{error}</span>
          </div>
        ) : null}
      </div>
    </div>
  );
}

type DiagnosticTone = 'ok' | 'warn' | 'muted';

function DiagnosticMetric({
  label,
  value,
  tone,
}: {
  label: string;
  value: string;
  tone: DiagnosticTone;
}): JSX.Element {
  return (
    <div className="flex items-center justify-between gap-3 rounded-lg border border-border bg-background px-3 py-2">
      <span className="text-xs text-muted-foreground">{label}</span>
      <span className="inline-flex min-w-0 items-center gap-1.5 text-xs font-medium text-foreground">
        {diagnosticToneIcon(tone)}
        <span className="truncate">{value}</span>
      </span>
    </div>
  );
}

function diagnosticToneIcon(tone: DiagnosticTone): JSX.Element | null {
  switch (tone) {
    case 'ok':
      return <CheckCircle2 className="size-3.5 shrink-0 text-emerald-500" />;
    case 'warn':
      return <AlertCircle className="size-3.5 shrink-0 text-amber-500" />;
    case 'muted':
      return null;
  }
}

function runtimeTone(status: string | undefined): DiagnosticTone {
  if (!status) return 'muted';
  if (status === 'ready') return 'ok';
  if (status === 'degraded') return 'warn';
  return 'muted';
}

function supervisorTone(status: string | undefined): DiagnosticTone {
  if (status === 'ready') return 'ok';
  if (status === 'starting' || status === 'restarting') return 'warn';
  return 'muted';
}

/* ── 关于我们（已迁移到 AboutTab.tsx） ── */
