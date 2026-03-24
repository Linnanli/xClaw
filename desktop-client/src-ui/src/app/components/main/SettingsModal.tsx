/**
 * SettingsModal - 设置弹窗
 *
 * 设计稿：900x700 模态框，左侧 200px 导航 + 右侧内容区。
 * 导航项：通用设置 / 用量统计 / 技能管理 / 扩展 / 记忆 / 关于我们
 */

import { useState } from 'react';
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

type SettingsNav =
  | 'general'
  | 'usage'
  | 'skills'
  | 'extensions'
  | 'memory'
  | 'about';

const NAV_ITEMS: { key: SettingsNav; label: string; icon: React.ElementType }[] = [
  { key: 'general', label: '通用设置', icon: Settings },
  { key: 'usage', label: '用量统计', icon: BarChart3 },
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
      case 'skills':
        return <SkillsTab />;
      case 'extensions':
        return <ExtensionsTab />;
      case 'memory':
        return <MemoryTab />;
      case 'about':
        return <AboutSection />;
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
          <div className="flex items-center justify-between border-b border-border px-6 py-4">
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
    case 'skills': return '管理已安装的技能和插件';
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

/* ── 关于我们 ── */
function AboutSection() {
  return (
    <div className="space-y-4">
      <div className="rounded-xl border border-border bg-card p-6 text-center">
        <Shield className="mx-auto mb-3 size-10 text-primary" />
        <h4 className="text-lg font-bold text-foreground">X-Claw</h4>
        <p className="mt-1 text-sm text-muted-foreground">
          企业级 AI 安全助手
        </p>
        <p className="mt-3 text-xs text-muted-foreground">版本 0.1.0</p>
      </div>
      <div className="rounded-xl border border-border bg-card p-4">
        <p className="text-xs text-muted-foreground">
          © 2024 X-Claw Team. All rights reserved.
        </p>
      </div>
    </div>
  );
}
