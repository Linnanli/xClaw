import React, { useState } from 'react';
import { MessageSquare, Brain, Briefcase, Calendar, Puzzle, Zap, FileText, Sun, Moon, Lock, User, Settings } from 'lucide-react';
import { ChatTabWithSSE } from '../tabs/ChatTabWithSSE';
import { MemoryTab } from '../tabs/MemoryTab';
import { JobsTab } from '../tabs/JobsTab';
import { RoutinesTab } from '../tabs/RoutinesTab';
import { ExtensionsTab } from '../tabs/ExtensionsTab';
import { SkillsTab } from '../tabs/SkillsTab';
import { LogsTab } from '../tabs/LogsTab';
import { SettingsTab } from '../tabs/SettingsTab';
import { useTheme } from '../../contexts/ThemeContext';
import { sessionApi } from '../../utils/tauri';
import { ShortcutManager, SHORTCUTS } from '../../utils/shortcuts';

type TabName = 'chat' | 'memory' | 'jobs' | 'routines' | 'extensions' | 'skills' | 'logs' | 'settings';

export function MainApp() {
  const [activeTab, setActiveTab] = useState<TabName>('chat');
  const [isConnected, setIsConnected] = useState(true);
  const [showUserMenu, setShowUserMenu] = useState(false);
  const { theme, toggleTheme } = useTheme();

  // Initialize shortcuts
  React.useEffect(() => {
    const shortcutManager = new ShortcutManager();
    
    shortcutManager.register({
      ...SHORTCUTS.LOCK_APP,
      action: handleLock,
    });

    shortcutManager.register({
      ...SHORTCUTS.NEW_THREAD,
      action: () => setActiveTab('chat'),
    });

    shortcutManager.register({
      ...SHORTCUTS.SEARCH,
      action: () => setActiveTab('chat'),
    });

    const handleKeyDown = (e: KeyboardEvent) => shortcutManager.handleKeyDown(e);
    window.addEventListener('keydown', handleKeyDown);

    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);

  const handleLock = async () => {
    try {
      await sessionApi.lockApp();
      // Redirect to login screen
      window.location.reload();
    } catch (err) {
      console.error('Failed to lock app:', err);
    }
  };

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
        return <ChatTabWithSSE />;
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
      case 'settings':
        return <SettingsTab />;
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

          {/* Settings Button */}
          <button
            onClick={() => setActiveTab('settings')}
            className={`flex items-center gap-2 px-4 py-3 border-b-2 transition-colors ${
              activeTab === 'settings'
                ? theme === 'dark'
                  ? 'border-[#5ddad5] text-white'
                  : 'border-[#667eea] text-[#667eea]'
                : theme === 'dark'
                  ? 'border-transparent text-gray-400 hover:text-white'
                  : 'border-transparent text-[#666] hover:text-[#667eea]'
            }`}
            title="设置"
          >
            <Settings size={18} />
            <span className="font-medium">设置</span>
          </button>

          {/* User Menu */}
          <div className="relative">
            <button
              onClick={() => setShowUserMenu(!showUserMenu)}
              className={`p-2 rounded-lg transition-colors ${
                theme === 'dark'
                  ? 'hover:bg-[#0a1628] text-gray-400 hover:text-[#5ddad5]'
                  : 'hover:bg-[#f5f5f5] text-[#666] hover:text-[#667eea]'
              }`}
              title="用户菜单"
            >
              <User size={20} />
            </button>
            
            {showUserMenu && (
              <div className={`absolute right-0 mt-2 w-48 rounded-lg shadow-lg z-50 ${
                theme === 'dark'
                  ? 'bg-[#0f1d35] border border-[#1a2942]'
                  : 'bg-white border border-[#ddd]'
              }`}>
                <button
                  onClick={handleLock}
                  className={`w-full text-left px-4 py-2 flex items-center gap-2 transition-colors ${
                    theme === 'dark'
                      ? 'hover:bg-[#1a2942] text-gray-300'
                      : 'hover:bg-[#f5f5f5] text-[#333]'
                  }`}
                >
                  <Lock size={16} />
                  <span>锁定应用</span>
                </button>
              </div>
            )}
          </div>

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