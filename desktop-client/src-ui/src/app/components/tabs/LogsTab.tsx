import { useState } from 'react';
import { Filter, Search, AlertCircle, Info, AlertTriangle, XCircle } from 'lucide-react';
import { useTheme } from '../../contexts/ThemeContext';

interface LogEntry {
  id: string;
  timestamp: Date;
  level: 'info' | 'warning' | 'error' | 'debug';
  message: string;
  module: string;
}

export function LogsTab() {
  const { theme } = useTheme();
  const [searchQuery, setSearchQuery] = useState('');
  const [filterLevel, setFilterLevel] = useState<string>('all');
  const [logs] = useState<LogEntry[]>([
    {
      id: '1',
      timestamp: new Date(),
      level: 'info',
      message: '应用启动成功',
      module: 'system',
    },
    {
      id: '2',
      timestamp: new Date(Date.now() - 30000),
      level: 'info',
      message: '用户登录成功',
      module: 'auth',
    },
    {
      id: '3',
      timestamp: new Date(Date.now() - 60000),
      level: 'warning',
      message: 'API调用延迟较高',
      module: 'network',
    },
    {
      id: '4',
      timestamp: new Date(Date.now() - 120000),
      level: 'error',
      message: '无法连接到远程服务器',
      module: 'network',
    },
    {
      id: '5',
      timestamp: new Date(Date.now() - 180000),
      level: 'debug',
      message: '加载扩展: Notion',
      module: 'extensions',
    },
  ]);

  const getLevelIcon = (level: string) => {
    switch (level) {
      case 'info':
        return <Info className="text-blue-400" size={18} />;
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
    switch (level) {
      case 'info':
        return <span className="px-2 py-1 bg-blue-400/10 text-blue-400 text-xs rounded border border-blue-400/30">信息</span>;
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

  const filteredLogs = logs.filter(log => {
    const matchesSearch = log.message.toLowerCase().includes(searchQuery.toLowerCase()) ||
                         log.module.toLowerCase().includes(searchQuery.toLowerCase());
    const matchesLevel = filterLevel === 'all' || log.level === filterLevel;
    return matchesSearch && matchesLevel;
  });

  return (
    <div className="h-full flex flex-col">
      <div className="p-6">
        <div className="mb-6">
          <h2 className={`text-2xl font-bold mb-4 ${theme === 'dark' ? 'text-white' : 'text-[#333]'}`}>日志</h2>
          
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
                <option value="info">信息</option>
                <option value="warning">警告</option>
                <option value="error">错误</option>
                <option value="debug">调试</option>
              </select>
            </div>
          </div>
        </div>

        <div className="space-y-2">
          {filteredLogs.map((log) => (
            <div
              key={log.id}
              className={`border rounded-lg p-4 transition-colors ${
                theme === 'dark'
                  ? 'bg-[#0f1d35] border-[#1a2942] hover:border-[#5ddad5]/30'
                  : 'bg-white border-[#ddd] hover:border-[#667eea]/50 shadow-sm hover:shadow-md'
              }`}
            >
              <div className="flex items-start gap-3">
                {getLevelIcon(log.level)}
                <div className="flex-1">
                  <div className="flex items-center gap-2 mb-1">
                    {getLevelBadge(log.level)}
                    <span className={`px-2 py-1 text-xs rounded border ${
                      theme === 'dark'
                        ? 'bg-[#0a1628] text-gray-400 border-[#1a2942]'
                        : 'bg-[#f5f5f5] text-[#666] border-[#eee]'
                    }`}>
                      {log.module}
                    </span>
                    <span className={`text-xs ${theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}`}>
                      {log.timestamp.toLocaleString('zh-CN')}
                    </span>
                  </div>
                  <p className={`text-sm ${theme === 'dark' ? 'text-white' : 'text-[#333]'}`}>{log.message}</p>
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
