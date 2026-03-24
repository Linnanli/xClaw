/**
 * LogsTab - 日志查看器
 *
 * 使用新主题 CSS 变量，支持搜索、级别过滤、模块过滤、导出和清空。
 * 复用 shadcn/ui 组件。
 */

import { useState, useEffect } from 'react';
import {
  Search,
  Filter,
  Info,
  AlertTriangle,
  XCircle,
  AlertCircle,
  Download,
  Trash2,
  Radio,
} from 'lucide-react';
import { Button } from '../ui/button';
import { ScrollArea } from '../ui/scroll-area';
import { cn } from '../ui/utils';
import { logApi, logClearApi, type LogEntry } from '../../utils/tauri';

const LEVEL_CONFIG: Record<string, { icon: React.ElementType; color: string; bg: string; label: string }> = {
  info: { icon: Info, color: 'text-blue-500', bg: 'bg-blue-50 dark:bg-blue-950/30', label: '信息' },
  warn: { icon: AlertTriangle, color: 'text-amber-500', bg: 'bg-amber-50 dark:bg-amber-950/30', label: '警告' },
  warning: { icon: AlertTriangle, color: 'text-amber-500', bg: 'bg-amber-50 dark:bg-amber-950/30', label: '警告' },
  error: { icon: XCircle, color: 'text-red-500', bg: 'bg-red-50 dark:bg-red-950/30', label: '错误' },
  debug: { icon: AlertCircle, color: 'text-muted-foreground', bg: 'bg-secondary', label: '调试' },
};

export function LogsTab() {
  const [searchQuery, setSearchQuery] = useState('');
  const [filterLevel, setFilterLevel] = useState('all');
  const [filterModule, setFilterModule] = useState('all');
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [streamEnabled, setStreamEnabled] = useState(false);

  useEffect(() => {
    fetchLogs();
  }, [searchQuery, filterLevel, filterModule]);

  const fetchLogs = async () => {
    try {
      setLoading(true);
      let fetched: LogEntry[];
      if (searchQuery.trim()) {
        fetched = await logApi.searchLogs(searchQuery, 100);
      } else if (filterLevel !== 'all') {
        fetched = await logApi.filterLogs(filterLevel, filterModule === 'all' ? '' : filterModule, 100);
      } else {
        fetched = await logApi.getLogs(100);
      }
      setLogs(fetched);
      setError(null);
    } catch (err) {
      console.error('Failed to fetch logs:', err);
      setError('加载日志失败');
      setLogs([]);
    } finally {
      setLoading(false);
    }
  };

  const handleExport = async () => {
    try {
      const data = await logApi.exportLogs('json');
      const blob = new Blob([data], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `logs-${new Date().toISOString()}.json`;
      a.click();
      URL.revokeObjectURL(url);
    } catch (err) {
      console.error('Failed to export logs:', err);
      setError('导出失败');
    }
  };

  const handleClear = async () => {
    if (confirm('确定要清空所有日志吗？此操作无法撤销。')) {
      try {
        await logClearApi.clearLogs();
        setLogs([]);
        setError(null);
      } catch (err) {
        console.error('Failed to clear logs:', err);
        setError('清空失败');
      }
    }
  };

  const filteredLogs = logs.filter((log) => {
    const q = searchQuery.toLowerCase();
    const matchSearch = !q || log.message.toLowerCase().includes(q) || log.module.toLowerCase().includes(q);
    const matchLevel = filterLevel === 'all' || log.level.toLowerCase() === filterLevel;
    const matchModule = filterModule === 'all' || log.module === filterModule;
    return matchSearch && matchLevel && matchModule;
  });

  const modules = Array.from(new Set(logs.map((l) => l.module)));

  if (loading && logs.length === 0) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="text-center text-muted-foreground">
          <AlertCircle className="mx-auto mb-3 size-10 animate-spin opacity-40" />
          <p className="text-sm">加载日志中...</p>
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col bg-background">
      {/* Toolbar */}
      <div className="border-b border-border px-6 py-4">
        <div className="mb-4 flex items-center justify-between">
          <h2 className="text-lg font-bold text-foreground">日志</h2>
          <div className="flex items-center gap-2">
            <Button
              variant={streamEnabled ? 'default' : 'outline'}
              size="sm"
              className="h-8 gap-1.5 text-xs"
              onClick={() => setStreamEnabled(!streamEnabled)}
            >
              <Radio className={cn('size-3', streamEnabled && 'animate-pulse')} />
              {streamEnabled ? '实时流：开' : '实时流：关'}
            </Button>
          </div>
        </div>

        {error && (
          <div className="mb-3 rounded-lg border border-destructive/30 bg-destructive/5 px-3 py-2 text-sm text-destructive">
            {error}
          </div>
        )}

        <div className="flex items-center gap-2">
          {/* Search */}
          <div className="relative flex-1">
            <Search className="absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
            <input
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="搜索日志..."
              className="w-full rounded-lg border border-border bg-secondary py-2 pl-9 pr-3 text-sm text-foreground placeholder:text-muted-foreground focus:border-primary focus:outline-none"
            />
          </div>

          {/* Level filter */}
          <div className="relative">
            <Filter className="absolute left-2.5 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground" />
            <select
              value={filterLevel}
              onChange={(e) => setFilterLevel(e.target.value)}
              className="rounded-lg border border-border bg-secondary py-2 pl-8 pr-3 text-sm text-foreground focus:border-primary focus:outline-none"
            >
              <option value="all">所有级别</option>
              <option value="debug">调试</option>
              <option value="info">信息</option>
              <option value="warn">警告</option>
              <option value="error">错误</option>
            </select>
          </div>

          {/* Module filter */}
          <select
            value={filterModule}
            onChange={(e) => setFilterModule(e.target.value)}
            className="rounded-lg border border-border bg-secondary py-2 pl-3 pr-3 text-sm text-foreground focus:border-primary focus:outline-none"
          >
            <option value="all">所有模块</option>
            {modules.map((m) => (
              <option key={m} value={m}>{m}</option>
            ))}
          </select>

          <Button variant="default" size="sm" className="h-9 gap-1.5" onClick={handleExport}>
            <Download className="size-3.5" />
            导出
          </Button>
          <Button variant="outline" size="sm" className="h-9 gap-1.5 border-destructive/30 text-destructive hover:bg-destructive/5" onClick={handleClear}>
            <Trash2 className="size-3.5" />
            清空
          </Button>
        </div>
      </div>

      {/* Log List */}
      <ScrollArea className="flex-1">
        <div className="space-y-2 p-6">
          {filteredLogs.length === 0 && (
            <div className="py-12 text-center text-sm text-muted-foreground">
              <AlertCircle className="mx-auto mb-3 size-8 opacity-40" />
              没有找到匹配的日志
            </div>
          )}
          {filteredLogs.map((log, i) => {
            const level = log.level.toLowerCase();
            const cfg = LEVEL_CONFIG[level] || LEVEL_CONFIG.debug;
            const Icon = cfg.icon;
            return (
              <div
                key={`${log.timestamp}-${i}`}
                className="rounded-xl border border-border bg-card p-4 transition-colors hover:border-primary/20"
              >
                <div className="flex items-start gap-3">
                  <Icon className={cn('mt-0.5 size-[18px]', cfg.color)} />
                  <div className="flex-1">
                    <div className="mb-1 flex flex-wrap items-center gap-2">
                      <span className={cn('rounded px-2 py-0.5 text-xs font-medium border', cfg.bg, cfg.color, 'border-current/20')}>
                        {cfg.label}
                      </span>
                      <span className="rounded bg-secondary px-2 py-0.5 text-xs text-text-secondary">
                        {log.module}
                      </span>
                      <span className="text-xs text-muted-foreground">
                        {new Date(log.timestamp).toLocaleString('zh-CN')}
                      </span>
                    </div>
                    <p className="text-sm text-foreground">{log.message}</p>
                    {log.context && (
                      <details className="mt-2 text-xs text-muted-foreground">
                        <summary className="cursor-pointer hover:text-foreground">详情</summary>
                        <pre className="mt-1 overflow-auto rounded-lg bg-secondary p-2 text-xs text-foreground">
                          {JSON.stringify(log.context, null, 2)}
                        </pre>
                      </details>
                    )}
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      </ScrollArea>
    </div>
  );
}
