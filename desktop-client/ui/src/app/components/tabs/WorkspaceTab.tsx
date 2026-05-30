/**
 * WorkspaceTab — 工作区状态面板
 *
 * 显示当前工作区的全局信息：
 * - Git 分支 / 文件变更状态
 * - LSP 诊断摘要（error / warning 计数）
 * - 当前活跃的 MCP / LSP 连接
 *
 * 数据来源：通过 invokeTauri 调用已有 git_status / lsp_query 等工具。
 */

import { useState, useEffect, useCallback } from 'react';
import {
  GitBranchIcon,
  AlertTriangleIcon,
  XCircleIcon,
  PlugIcon,
  RefreshCwIcon,
  FolderOpenIcon,
} from 'lucide-react';
import { cn } from '../ui/utils';
import { invokeTauri } from '../../utils/tauri';

// ── 类型定义 ──────────────────────────────────────────────────────

interface GitStatus {
  branch: string;
  modified: number;
  staged: number;
  untracked: number;
}

interface DiagnosticSummary {
  errors: number;
  warnings: number;
}

interface ActiveServer {
  name: string;
  type: 'lsp' | 'mcp';
}

// ── 子组件 ────────────────────────────────────────────────────────

function StatusCard({
  icon: Icon,
  title,
  children,
  className,
}: {
  icon: React.ElementType;
  title: string;
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <div className={cn('rounded-lg border border-border/50 bg-secondary/30 p-3', className)}>
      <div className="mb-2 flex items-center gap-2">
        <Icon className="h-4 w-4 text-muted-foreground" />
        <span className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
          {title}
        </span>
      </div>
      {children}
    </div>
  );
}

function StatBadge({ value, label, variant }: { value: number; label: string; variant: 'error' | 'warn' | 'info' | 'muted' }) {
  const colorMap = {
    error: 'text-red-400 bg-red-400/10',
    warn: 'text-yellow-400 bg-yellow-400/10',
    info: 'text-blue-400 bg-blue-400/10',
    muted: 'text-muted-foreground bg-muted/50',
  };
  return (
    <span className={cn('inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-xs font-medium', colorMap[variant])}>
      {value} {label}
    </span>
  );
}

// ── 主组件 ────────────────────────────────────────────────────────

export function WorkspaceTab() {
  const [gitStatus, setGitStatus] = useState<GitStatus | null>(null);
  const [diagnostics, setDiagnostics] = useState<DiagnosticSummary>({ errors: 0, warnings: 0 });
  const [activeServers, setActiveServers] = useState<ActiveServer[]>([]);
  const [workspaceRoot, setWorkspaceRoot] = useState<string>('');
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadWorkspaceState = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);

      // Fetch git status via existing Tauri tool dispatch
      try {
        const gitResult = await invokeTauri<string>('ic_workspace_git_status');
        const parsed = parseGitStatus(gitResult);
        setGitStatus(parsed);
      } catch {
        // Git not available in this workspace
        setGitStatus(null);
      }

      // Fetch workspace root
      try {
        const root = await invokeTauri<string>('ic_workspace_root');
        setWorkspaceRoot(root);
      } catch {
        setWorkspaceRoot('');
      }

      // Fetch active servers
      try {
        const servers = await invokeTauri<ActiveServer[]>('ic_active_servers');
        setActiveServers(servers);
      } catch {
        setActiveServers([]);
      }
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadWorkspaceState();
  }, [loadWorkspaceState]);

  return (
    <div className="flex h-full flex-col gap-4 p-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <FolderOpenIcon className="h-5 w-5 text-blue-400" />
          <h2 className="text-sm font-semibold text-foreground">工作区</h2>
          {workspaceRoot && (
            <span className="text-xs text-muted-foreground truncate max-w-[300px]">
              {workspaceRoot}
            </span>
          )}
        </div>
        <button
          type="button"
          onClick={loadWorkspaceState}
          disabled={loading}
          className="rounded p-1 text-muted-foreground hover:bg-muted/50 hover:text-foreground"
        >
          <RefreshCwIcon className={cn('h-4 w-4', loading && 'animate-spin')} />
        </button>
      </div>

      {error && (
        <div className="rounded-md border border-red-500/30 bg-red-500/10 px-3 py-2 text-xs text-red-400">
          {error}
        </div>
      )}

      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
        {/* Git Status */}
        <StatusCard icon={GitBranchIcon} title="Git">
          {gitStatus ? (
            <div className="space-y-1.5">
              <div className="text-sm font-medium text-foreground">{gitStatus.branch}</div>
              <div className="flex flex-wrap gap-1.5">
                {gitStatus.staged > 0 && (
                  <StatBadge value={gitStatus.staged} label="staged" variant="info" />
                )}
                {gitStatus.modified > 0 && (
                  <StatBadge value={gitStatus.modified} label="modified" variant="warn" />
                )}
                {gitStatus.untracked > 0 && (
                  <StatBadge value={gitStatus.untracked} label="untracked" variant="muted" />
                )}
                {gitStatus.staged === 0 && gitStatus.modified === 0 && gitStatus.untracked === 0 && (
                  <span className="text-xs text-muted-foreground">Clean</span>
                )}
              </div>
            </div>
          ) : (
            <span className="text-xs text-muted-foreground">Not a git repository</span>
          )}
        </StatusCard>

        {/* Diagnostics */}
        <StatusCard icon={AlertTriangleIcon} title="诊断">
          <div className="flex items-center gap-2">
            {diagnostics.errors > 0 ? (
              <div className="flex items-center gap-1">
                <XCircleIcon className="h-3.5 w-3.5 text-red-400" />
                <StatBadge value={diagnostics.errors} label="errors" variant="error" />
              </div>
            ) : null}
            {diagnostics.warnings > 0 ? (
              <StatBadge value={diagnostics.warnings} label="warnings" variant="warn" />
            ) : null}
            {diagnostics.errors === 0 && diagnostics.warnings === 0 && (
              <span className="text-xs text-muted-foreground">No issues</span>
            )}
          </div>
        </StatusCard>

        {/* Active Servers */}
        <StatusCard icon={PlugIcon} title="活跃连接" className="sm:col-span-2">
          {activeServers.length > 0 ? (
            <div className="flex flex-wrap gap-2">
              {activeServers.map((server) => (
                <span
                  key={`${server.type}-${server.name}`}
                  className="inline-flex items-center gap-1.5 rounded-full border border-border/50 px-2.5 py-1 text-xs"
                >
                  <span
                    className={cn(
                      'h-1.5 w-1.5 rounded-full',
                      server.type === 'lsp' ? 'bg-purple-400' : 'bg-green-400',
                    )}
                  />
                  <span className="text-foreground">{server.name}</span>
                  <span className="text-muted-foreground uppercase">{server.type}</span>
                </span>
              ))}
            </div>
          ) : (
            <span className="text-xs text-muted-foreground">No active servers</span>
          )}
        </StatusCard>
      </div>
    </div>
  );
}

// ── 辅助函数 ──────────────────────────────────────────────────────

function parseGitStatus(raw: string): GitStatus {
  let branch = 'unknown';
  let modified = 0;
  let staged = 0;
  let untracked = 0;

  for (const line of raw.split('\n')) {
    const branchMatch = line.match(/^On branch (.+)/);
    if (branchMatch) {
      branch = branchMatch[1];
      continue;
    }
    if (line.match(/^\s*modified:/)) modified++;
    if (line.match(/^\s*new file:/)) staged++;
    if (line.match(/^\?\?/)) untracked++;
    // Porcelain format: first char = staged, second = working tree
    if (line.length >= 3 && line[2] === ' ') {
      const idx = line[0];
      const wt = line[1];
      if (idx !== ' ' && idx !== '?') staged++;
      if (wt !== ' ' && wt !== '?') modified++;
      if (idx === '?' && wt === '?') untracked++;
    }
  }

  return { branch, modified, staged, untracked };
}
