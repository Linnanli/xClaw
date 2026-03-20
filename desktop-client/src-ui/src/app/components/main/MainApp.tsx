import React, { useState, useEffect } from 'react';
import { MessageSquare, Brain, Briefcase, Calendar, Puzzle, Zap, FileText, Sun, Moon, Monitor, Lock, User, Settings } from 'lucide-react';
import { ChatTabTauri } from '../tabs/ChatTabTauri';
import { MemoryTab } from '../tabs/MemoryTab';
import { JobsTab } from '../tabs/JobsTab';
import { RoutinesTab } from '../tabs/RoutinesTab';
import { ExtensionsTab } from '../tabs/ExtensionsTab';
import { SkillsTab } from '../tabs/SkillsTab';
import { LogsTab } from '../tabs/LogsTab';
import { SettingsTab } from '../tabs/SettingsTab';
import { ConnectionStatus } from '../common/ConnectionStatus';
import { DynamicWatermark } from '../common/DynamicWatermark';
import { useTheme } from '../../contexts/ThemeContext';
import { useSSEConnection } from '../../hooks/useSSEConnection';
import { useWatermark } from '../../hooks/useWatermark';
import { sessionApi, appApi } from '../../utils/tauri';
import { ShortcutManager, SHORTCUTS } from '../../utils/shortcuts';
import { tracing } from '../../utils/tracing';

type TabName = 'chat' | 'memory' | 'jobs' | 'routines' | 'extensions' | 'skills' | 'logs' | 'settings';

export function MainApp() {
  const [activeTab, setActiveTab] = useState<TabName>('chat');
  const [showUserMenu, setShowUserMenu] = useState(false);
  const [showThemeMenu, setShowThemeMenu] = useState(false);
  const { theme, themeMode, setTheme } = useTheme();
  const { connect, disconnect } = useSSEConnection();
  const { config: watermarkConfig, loading: watermarkLoading } = useWatermark();

  // Initialize SSE connection
  useEffect(() => {
    const initializeConnection = async () => {
      try {
        // 获取实际的认证信息
        const appInfo = await appApi.getAppInitInfo();
        const fullToken = await appApi.getAuthToken();
        
        await connect(fullToken, appInfo.api_base_url);
        tracing.info('SSE connection initialized in MainApp', { 
          baseUrl: appInfo.api_base_url,
          tokenLength: fullToken.length 
        });
      } catch (err) {
        tracing.error('Failed to initialize SSE connection', { error: err });
      }
    };

    initializeConnection();

    // 清理连接
    return () => {
      disconnect();
    };
  }, [connect, disconnect]);

  // Close menus when clicking outside
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      const target = event.target as Element;
      
      // Close theme menu if clicking outside
      if (showThemeMenu && !target.closest('[data-theme-menu]')) {
        setShowThemeMenu(false);
      }
      
      // Close user menu if clicking outside
      if (showUserMenu && !target.closest('[data-user-menu]')) {
        setShowUserMenu(false);
      }
    };

    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [showThemeMenu, showUserMenu]);

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
        return <ChatTabTauri />;
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
          <div className="relative" data-theme-menu>
            <button
              onClick={() => setShowThemeMenu(!showThemeMenu)}
              className={`p-2 rounded-lg transition-colors ${
                theme === 'dark'
                  ? 'hover:bg-[#0a1628] text-gray-400 hover:text-[#5ddad5]'
                  : 'hover:bg-[#f5f5f5] text-[#666] hover:text-[#667eea]'
              }`}
              title="主题设置"
            >
              {themeMode === 'system' ? (
                <Monitor size={20} />
              ) : theme === 'dark' ? (
                <Moon size={20} />
              ) : (
                <Sun size={20} />
              )}
            </button>
            
            {showThemeMenu && (
              <div className={`absolute right-0 mt-2 w-40 rounded-lg shadow-lg z-50 ${
                theme === 'dark'
                  ? 'bg-[#0f1d35] border border-[#1a2942]'
                  : 'bg-white border border-[#ddd]'
              }`}>
                <button
                  onClick={() => {
                    setTheme('light');
                    setShowThemeMenu(false);
                  }}
                  className={`w-full text-left px-4 py-2 flex items-center gap-2 transition-colors ${
                    themeMode === 'light'
                      ? theme === 'dark'
                        ? 'bg-[#1a2942] text-[#5ddad5]'
                        : 'bg-[#f0f0f0] text-[#667eea]'
                      : theme === 'dark'
                        ? 'hover:bg-[#1a2942] text-gray-300'
                        : 'hover:bg-[#f5f5f5] text-[#333]'
                  }`}
                >
                  <Sun size={16} />
                  <span>浅色</span>
                </button>
                <button
                  onClick={() => {
                    setTheme('dark');
                    setShowThemeMenu(false);
                  }}
                  className={`w-full text-left px-4 py-2 flex items-center gap-2 transition-colors ${
                    themeMode === 'dark'
                      ? theme === 'dark'
                        ? 'bg-[#1a2942] text-[#5ddad5]'
                        : 'bg-[#f0f0f0] text-[#667eea]'
                      : theme === 'dark'
                        ? 'hover:bg-[#1a2942] text-gray-300'
                        : 'hover:bg-[#f5f5f5] text-[#333]'
                  }`}
                >
                  <Moon size={16} />
                  <span>深色</span>
                </button>
                <button
                  onClick={() => {
                    setTheme('system');
                    setShowThemeMenu(false);
                  }}
                  className={`w-full text-left px-4 py-2 flex items-center gap-2 transition-colors ${
                    themeMode === 'system'
                      ? theme === 'dark'
                        ? 'bg-[#1a2942] text-[#5ddad5]'
                        : 'bg-[#f0f0f0] text-[#667eea]'
                      : theme === 'dark'
                        ? 'hover:bg-[#1a2942] text-gray-300'
                        : 'hover:bg-[#f5f5f5] text-[#333]'
                  }`}
                >
                  <Monitor size={16} />
                  <span>跟随系统</span>
                </button>
              </div>
            )}
          </div>

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
          <div className="relative" data-user-menu>
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
          <ConnectionStatus />
        </div>
      </div>

      {/* Tab Content */}
      <div className="flex-1 overflow-hidden">
        {renderTabContent()}
      </div>

      {/* Dynamic Watermark */}
      {!watermarkLoading && (
        <DynamicWatermark
          text={watermarkConfig.text}
          enabled={watermarkConfig.enabled}
          opacity={watermarkConfig.opacity}
          fontSize={watermarkConfig.fontSize}
          rotation={watermarkConfig.rotation}
          spacing={watermarkConfig.spacing}
        />
      )}
    </div>
  );
}