import { useState } from 'react';
import { Search, FileText, FolderTree } from 'lucide-react';
import { useTheme } from '../../contexts/ThemeContext';

interface MemoryEntry {
  id: string;
  path: string;
  content: string;
  updatedAt: Date;
}

export function MemoryTab() {
  const { theme } = useTheme();
  const [searchQuery, setSearchQuery] = useState('');
  const [entries] = useState<MemoryEntry[]>([
    {
      id: '1',
      path: '/projects/ironclaw/README.md',
      content: 'IronClaw项目文档...',
      updatedAt: new Date(),
    },
    {
      id: '2',
      path: '/notes/meeting-notes.md',
      content: '团队会议记录...',
      updatedAt: new Date(Date.now() - 86400000),
    },
    {
      id: '3',
      path: '/ideas/feature-requests.md',
      content: '功能请求列表...',
      updatedAt: new Date(Date.now() - 172800000),
    },
  ]);

  return (
    <div className="h-full flex flex-col">
      <div className="p-6">
        <div className="flex items-center gap-4 mb-6">
          <h2 className={`text-2xl font-bold ${theme === 'dark' ? 'text-white' : 'text-[#333]'}`}>记忆管理</h2>
        </div>

        <div className="mb-6 flex gap-3">
          <div className="flex-1 relative">
            <Search className={`absolute left-3 top-1/2 -translate-y-1/2 ${theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}`} size={20} />
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="搜索记忆..."
              className={`w-full pl-10 pr-4 py-2 border rounded-lg focus:outline-none ${
                theme === 'dark'
                  ? 'bg-[#0f1d35] border-[#1a2942] focus:border-[#5ddad5] text-white placeholder-gray-500'
                  : 'bg-white border-[#ddd] focus:border-[#667eea] focus:ring-2 focus:ring-[#667eea]/20 text-[#333] placeholder-gray-400'
              }`}
            />
          </div>
          <button className={`px-4 py-2 rounded-lg flex items-center gap-2 font-medium transition-opacity ${
            theme === 'dark'
              ? 'bg-gradient-to-r from-[#5ddad5] to-[#4facf7] text-[#0a1628] hover:opacity-90'
              : 'bg-[#667eea] text-white hover:opacity-90 shadow-md'
          }`}>
            <FolderTree size={18} />
            查看树形结构
          </button>
        </div>

        <div className="space-y-3">
          {entries.map((entry) => (
            <div
              key={entry.id}
              className={`border rounded-xl p-5 transition-colors cursor-pointer ${
                theme === 'dark'
                  ? 'bg-[#0f1d35] border-[#1a2942] hover:border-[#5ddad5]/30'
                  : 'bg-white border-[#ddd] hover:border-[#667eea]/50 shadow-sm hover:shadow-md'
              }`}
            >
              <div className="flex items-start gap-3">
                <FileText className={theme === 'dark' ? 'text-[#5ddad5]' : 'text-[#667eea]'} size={20} />
                <div className="flex-1">
                  <h3 className={`font-semibold mb-1 ${theme === 'dark' ? 'text-white' : 'text-[#333]'}`}>{entry.path}</h3>
                  <p className={`text-sm mb-2 ${theme === 'dark' ? 'text-gray-400' : 'text-[#666]'}`}>{entry.content}</p>
                  <p className={`text-xs ${theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}`}>
                    更新于: {entry.updatedAt.toLocaleString('zh-CN')}
                  </p>
                </div>
              </div>
            </div>
          ))}
        </div>

        {entries.length === 0 && (
          <div className={`text-center py-12 ${theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}`}>
            <FileText size={48} className="mx-auto mb-4 opacity-50" />
            <p>暂无记忆条目</p>
          </div>
        )}
      </div>
    </div>
  );
}
