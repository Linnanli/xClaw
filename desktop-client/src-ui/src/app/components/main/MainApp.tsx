import { useState } from 'react';
import { MessageSquare, Brain, Briefcase, Calendar, Puzzle, Zap, FileText, Sun, Moon } from 'lucide-react';
import { ChatTab } from '../tabs/ChatTab';
import { MemoryTab } from '../tabs/MemoryTab';
import { JobsTab } from '../tabs/JobsTab';
import { RoutinesTab } from '../tabs/RoutinesTab';
import { ExtensionsTab } from '../tabs/ExtensionsTab';
import { SkillsTab } from '../tabs/SkillsTab';
import { LogsTab } from '../tabs/LogsTab';
import { useTheme } from '../../contexts/ThemeContext';

type TabName = 'chat' | 'memory' | 'jobs' | 'routines' | 'extensions' | 'skills' | 'logs';

export function MainApp() {
  const [activeTab, setActiveTab] = useState<TabName>('chat');
  const [isConnected, setIsConnected] = useState(true);
  const { theme, toggleTheme } = useTheme();

  const tabs = [
    { id: 'chat' as const, label: '聊天', icon: MessageSquare },
    { id: 'memory' as const, label: '记忆', icon: Brain },
    { id: 'jobs' as const, label: '任务', icon: Briefcase },
    { id: 'routines' as const, label: '日程', icon: Calendar },
    { id: 'extensions' as const, label: '扩展', icon: Puzzle },
    { id: 'skills' as const, label: '技能', icon: Zap },
  ];

  const renderTabContent = () => {
    switch (activeTab) {
      case 'chat':
        return <ChatTab />;
      case 'memory':
        return <MemoryTab />;
      case 'jobs':
        return <JobsTab />;
      case 'routines':
        return <RoutinesTab />;
      case 'extensions':
        return <ExtensionsTab />;
      case 'skills':
        return <SkillsTab />;
      case 'logs':
        return <LogsTab />;
      default:
        return null;
    }
  };

  return (
    <div className={`h-screen flex flex-col ${
      theme === 'dark' ? 'bg-[#0a1628]' : 'bg-[#f5f5f5]'
    }`}>
      {/* Tab Navigation */}
      <div className={`${
        theme === 'dark' 
          ? 'bg-[#0f1d35] border-b border-[#1a2942]' 
          : 'bg-white border-b border-[#ddd]'
      } flex items-center justify-between px-4`}>
        <div className="flex gap-1">
          {tabs.map((tab) => {
            const Icon = tab.icon;
            return (
              <button
                key={tab.id}
                onClick={() => setActiveTab(tab.id)}
                className={`flex items-center gap-2 px-4 py-3 border-b-2 transition-colors ${
                  activeTab === tab.id
                    ? theme === 'dark'
                      ? 'border-[#5ddad5] text-white'
                      : 'border-[#667eea] text-[#667eea]'
                    : theme === 'dark'
                      ? 'border-transparent text-gray-400 hover:text-white'
                      : 'border-transparent text-[#666] hover:text-[#667eea]'
                }`}
              >
                <Icon size={18} />
                <span className="font-medium">{tab.label}</span>
              </button>
            );
          })}
        </div>

        <div className="flex items-center gap-6">
          {/* Theme Toggle */}
          <button
            onClick={toggleTheme}
            className={`p-2 rounded-lg transition-colors ${
              theme === 'dark'
                ? 'hover:bg-[#0a1628] text-gray-400 hover:text-[#5ddad5]'
                : 'hover:bg-[#f5f5f5] text-[#666] hover:text-[#667eea]'
            }`}
            title={theme === 'dark' ? '切换到日间模式' : '切换到夜间模式'}
          >
            {theme === 'dark' ? <Sun size={20} /> : <Moon size={20} />}
          </button>

          {/* Logs Tab (right side) */}
          <button
            onClick={() => setActiveTab('logs')}
            className={`flex items-center gap-2 px-4 py-3 border-b-2 transition-colors ${
              activeTab === 'logs'
                ? theme === 'dark'
                  ? 'border-[#5ddad5] text-white'
                  : 'border-[#667eea] text-[#667eea]'
                : theme === 'dark'
                  ? 'border-transparent text-gray-400 hover:text-white'
                  : 'border-transparent text-[#666] hover:text-[#667eea]'
            }`}
          >
            <FileText size={18} />
            <span className="font-medium">日志</span>
          </button>

          {/* Connection Status */}
          <div className={`flex items-center gap-2 px-3 py-1.5 rounded-full ${
            theme === 'dark'
              ? 'bg-[#0a1628] border border-[#1a2942]'
              : 'bg-[#f5f5f5] border border-[#ddd]'
          }`}>
            <div className={`w-2 h-2 rounded-full ${isConnected ? 'bg-green-400' : 'bg-red-400'}`} />
            <span className={`text-sm ${theme === 'dark' ? 'text-gray-400' : 'text-[#666]'}`}>
              {isConnected ? '已连接' : '已断开'}
            </span>
          </div>
        </div>
      </div>

      {/* Tab Content */}
      <div className="flex-1 overflow-hidden">
        {renderTabContent()}
      </div>
    </div>
  );
}