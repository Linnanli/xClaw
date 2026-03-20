import { useState, useEffect } from 'react';
import { Filter, Search, AlertCircle, Info, AlertTriangle, XCircle, Download, Trash2 } from 'lucide-react';
import { useTheme } from '@contexts/ThemeContext';
import { logApi, logClearApi, LogEntry } from '@utils/tauri';
import { LogStreamClient } from '@utils/sse';
import { API_BASE_URL } from '@config/api';

export function LogsTab() {
  const { theme } = useTheme();
  const [searchQuery, setSearchQuery] = useState('');
  const [filterLevel, setFilterLevel] = useState<string>('all');
  const [filterModule, setFilterModule] = useState<string>('all');
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [streamEnabled, setStreamEnabled] = useState(false);

  useEffect(() => {
    const fetchLogs = async () => {
      try {
        setLoading(true);
        let fetchedLogs: LogEntry[];
        
        if (searchQuery.trim()) {
          fetchedLogs = await logApi.searchLogs(searchQuery, 100);
        } else if (filterLevel !== 'all' && filterModule !== 'all') {
          fetchedLogs = await logApi.filterLogs(filterLevel, filterModule, 100);
        } else if (filterLevel !== 'all') {
          fetchedLogs = await logApi.filterLogs(filterLevel, '', 100);
        } else {
          fetchedLogs = await logApi.getLogs(100);
        }
        
        setLogs(fetchedLogs);
        setError(null);
      } catch (err) {
        console.error('Failed to fetch logs:', err);
        setError('Failed to load logs');
        setLogs([]);
      } finally {
        setLoading(false);
      }
    };

    fetchLogs();
  }, [searchQuery, filterLevel, filterModule]);

  // Setup real-time log streaming
  useEffect(() => {
    if (!streamEnabled) return;

    const streamClient = new LogStreamClient(API_BASE_URL, (logEntry) => {
      setLogs((prevLogs) => [logEntry, ...prevLogs.slice(0, 99)]);
    });

    streamClient.connect();

    return () => streamClient.disconnect();
  }, [streamEnabled]);

  const getLevelIcon = (level: string) => {
    const levelLower = level.toLowerCase();
    switch (levelLower) {
      case 'info':
        return <Info className="text-blue-400" size={18} />;
      case 'warn':
      case 'warning':
        return <AlertTriangle className="text-yellow-400" size={18} />;
      case 'error':
        return <XCircle className="text-red-400" size={18} />;
      case 'debug':
        return <AlertCircle className="text-gray-400" size={18} />;
      default:
        return <Info className="text-gray-400" size={18} />;
    }
  };

  const getLevelBadge = (level: string) => {
    const levelLower = level.toLowerCase();
    switch (levelLower) {
      case 'info':
        return <span className="px-2 py-1 bg-blue-400/10 text-blue-400 text-xs rounded border border-blue-400/30">信息</span>;
      case 'warn':
      case 'warning':
        return <span className="px-2 py-1 bg-yellow-400/10 text-yellow-400 text-xs rounded border border-yellow-400/30">警告</span>;
      case 'error':
        return <span className="px-2 py-1 bg-red-400/10 text-red-400 text-xs rounded border border-red-400/30">错误</span>;
      case 'debug':
        return <span className="px-2 py-1 bg-gray-400/10 text-gray-400 text-xs rounded border border-gray-400/30">调试</span>;
      default:
        return null;
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
      setError('Failed to export logs');
    }
  };

  const handleClearLogs = async () => {
    if (confirm('确定要清空所有日志吗？此操作无法撤销。')) {
      try {
        await logClearApi.clearLogs();
        setLogs([]);
        setError(null);
      } catch (err) {
        console.error('Failed to clear logs:', err);
        setError('Failed to clear logs');
      }
    }
  };

  const filteredLogs = logs.filter(log => {
    const matchesSearch = log.message.toLowerCase().includes(searchQuery.toLowerCase()) ||
                         log.module.toLowerCase().includes(searchQuery.toLowerCase());
    const matchesLevel = filterLevel === 'all' || log.level.toLowerCase() === filterLevel.toLowerCase();
    const matchesModule = filterModule === 'all' || log.module === filterModule;
    return matchesSearch && matchesLevel && matchesModule;
  });

  // Extract unique modules from logs
  const modules = Array.from(new Set(logs.map(log => log.module)));

  if (loading) {
    return (
      <div className="h-full flex items-center justify-center">
        <div className={`text-center ${theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}`}>
          <div className="animate-spin mb-4">
            <AlertCircle size={48} className="mx-auto opacity-50" />
          </div>
          <p>加载日志中...</p>
        </div>
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col">
      <div className="p-6">
        <div className="mb-6">
          <h2 className={`text-2xl font-bold mb-4 ${theme === 'dark' ? 'text-white' : 'text-[#333]'}`}>日志</h2>
          {error && (
            <div className="mb-4 p-3 bg-red-400/10 border border-red-400/30 rounded-lg text-red-400 text-sm">
              {error}
            </div>
          )}
          
          <div className="flex gap-3">
            <div className="flex-1 relative">
              <Search className={`absolute left-3 top-1/2 -translate-y-1/2 ${theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}`} size={20} />
              <input
                type="text"
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                placeholder="搜索日志..."
                className={`w-full pl-10 pr-4 py-2 border rounded-lg focus:outline-none ${
                  theme === 'dark'
                    ? 'bg-[#0f1d35] border-[#1a2942] focus:border-[#5ddad5] text-white placeholder-gray-500'
                    : 'bg-white border-[#ddd] focus:border-[#667eea] focus:ring-2 focus:ring-[#667eea]/20 text-[#333] placeholder-gray-400'
                }`}
              />
            </div>
            <div className="relative">
              <Filter className={`absolute left-3 top-1/2 -translate-y-1/2 ${theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}`} size={20} />
              <select
                value={filterLevel}
                onChange={(e) => setFilterLevel(e.target.value)}
                className={`pl-10 pr-4 py-2 border rounded-lg focus:outline-none ${
                  theme === 'dark'
                    ? 'bg-[#0f1d35] border-[#1a2942] focus:border-[#5ddad5] text-white'
                    : 'bg-white border-[#ddd] focus:border-[#667eea] focus:ring-2 focus:ring-[#667eea]/20 text-[#333]'
                }`}
              >
                <option value="all">所有级别</option>
                <option value="debug">调试</option>
                <option value="info">信息</option>
                <option value="warn">警告</option>
                <option value="error">错误</option>
              </select>
            </div>
            <div className="relative">
              <Filter className={`absolute left-3 top-1/2 -translate-y-1/2 ${theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}`} size={20} />
              <select
                value={filterModule}
                onChange={(e) => setFilterModule(e.target.value)}
                className={`pl-10 pr-4 py-2 border rounded-lg focus:outline-none ${
                  theme === 'dark'
                    ? 'bg-[#0f1d35] border-[#1a2942] focus:border-[#5ddad5] text-white'
                    : 'bg-white border-[#ddd] focus:border-[#667eea] focus:ring-2 focus:ring-[#667eea]/20 text-[#333]'
                }`}
              >
                <option value="all">所有模块</option>
                {modules.map(module => (
                  <option key={module} value={module}>{module}</option>
                ))}
              </select>
            </div>
            <button
              onClick={handleExport}
              className={`px-4 py-2 rounded-lg flex items-center gap-2 font-medium transition-opacity ${
                theme === 'dark'
                  ? 'bg-gradient-to-r from-[#5ddad5] to-[#4facf7] text-[#0a1628] hover:opacity-90'
                  : 'bg-[#667eea] text-white hover:opacity-90 shadow-md'
              }`}
            >
              <Download size={18} />
              导出
            </button>
            <button
              onClick={handleClearLogs}
              className={`px-4 py-2 rounded-lg flex items-center gap-2 font-medium transition-opacity ${
                theme === 'dark'
                  ? 'bg-red-500/20 text-red-400 hover:bg-red-500/30 border border-red-500/30'
                  : 'bg-red-100 text-red-600 hover:bg-red-200 shadow-md'
              }`}
            >
              <Trash2 size={18} />
              清空
            </button>
            <button
              onClick={() => setStreamEnabled(!streamEnabled)}
              className={`px-4 py-2 rounded-lg flex items-center gap-2 font-medium transition-opacity ${
                streamEnabled
                  ? theme === 'dark'
                    ? 'bg-green-500/20 text-green-400 border border-green-500/30'
                    : 'bg-green-100 text-green-600 shadow-md'
                  : theme === 'dark'
                    ? 'bg-gray-500/20 text-gray-400 border border-gray-500/30'
                    : 'bg-gray-100 text-gray-600 shadow-md'
              }`}
            >
              <span className={`w-2 h-2 rounded-full ${streamEnabled ? 'bg-green-400' : 'bg-gray-400'}`} />
              {streamEnabled ? '实时流：开' : '实时流：关'}
            </button>
          </div>
        </div>

        <div className="space-y-2">
          {filteredLogs.map((log, index) => (
            <div
              key={`${log.timestamp}-${index}`}
              className={`border rounded-lg p-4 transition-colors ${
                theme === 'dark'
                  ? 'bg-[#0f1d35] border-[#1a2942] hover:border-[#5ddad5]/30'
                  : 'bg-white border-[#ddd] hover:border-[#667eea]/50 shadow-sm hover:shadow-md'
              }`}
            >
              <div className="flex items-start gap-3">
                {getLevelIcon(log.level)}
                <div className="flex-1">
                  <div className="flex items-center gap-2 mb-1 flex-wrap">
                    {getLevelBadge(log.level)}
                    <span className={`px-2 py-1 text-xs rounded border ${
                      theme === 'dark'
                        ? 'bg-[#0a1628] text-gray-400 border-[#1a2942]'
                        : 'bg-[#f5f5f5] text-[#666] border-[#eee]'
                    }`}>
                      {log.module}
                    </span>
                    <span className={`text-xs ${theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}`}>
                      {new Date(log.timestamp).toLocaleString('zh-CN')}
                    </span>
                  </div>
                  <p className={`text-sm ${theme === 'dark' ? 'text-white' : 'text-[#333]'}`}>{log.message}</p>
                  {log.context && (
                    <details className={`text-xs mt-2 ${theme === 'dark' ? 'text-gray-400' : 'text-[#666]'}`}>
                      <summary className="cursor-pointer">详情</summary>
                      <pre className={`mt-2 p-2 rounded overflow-auto ${
                        theme === 'dark'
                          ? 'bg-[#0a1628] text-gray-300'
                          : 'bg-[#f5f5f5] text-[#333]'
                      }`}>
                        {JSON.stringify(log.context, null, 2)}
                      </pre>
                    </details>
                  )}
                </div>
              </div>
            </div>
          ))}
        </div>

        {filteredLogs.length === 0 && (
          <div className={`text-center py-12 ${theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}`}>
            <AlertCircle size={48} className="mx-auto mb-4 opacity-50" />
            <p>没有找到匹配的日志</p>
          </div>
        )}
      </div>
    </div>
  );
}
