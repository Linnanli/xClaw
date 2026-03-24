/**
 * MainApp - 应用主布局
 *
 * 设计稿布局：左侧 240px Sidebar + 右侧 MainContent（Header + Content）。
 * 使用 shadcn/ui SidebarProvider 管理侧边栏状态。
 */

import React, { useState, useEffect } from 'react';
import { SidebarProvider, SidebarInset } from '../ui/sidebar';
import { AppSidebar, type NavItem } from './AppSidebar';
import { AppHeader } from './AppHeader';
import { ChatTabTauri } from '../tabs/ChatTabTauri';
import { LogsTab } from '../tabs/LogsTab';
import { RoutinesTab } from '../tabs/RoutinesTab';
import { SettingsModal } from './SettingsModal';
import { JobsPanel } from './JobsPanel';
import { NotificationsPanel } from './NotificationsPanel';
import { DynamicWatermark } from '../common/DynamicWatermark';
import { useWatermark } from '../../hooks/useWatermark';
import { sessionApi } from '../../utils/tauri';
import { ShortcutManager, SHORTCUTS } from '../../utils/shortcuts';
import { tracing } from '../../utils/tracing';

const NAV_TITLES: Record<NavItem, string> = {
  chat: '聊天',
  logs: '日志',
  routines: '定时任务',
  settings: '设置',
};

export function MainApp() {
  const [activeNav, setActiveNav] = useState<NavItem>('chat');
  const [selectedThreadId, setSelectedThreadId] = useState<string | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [jobsOpen, setJobsOpen] = useState(false);
  const [notificationsOpen, setNotificationsOpen] = useState(false);
  const [routinesOpen, setRoutinesOpen] = useState(false);
  const { config: watermarkConfig, loading: watermarkLoading } = useWatermark();

  // 嵌入式模式：SSE 连接跳过，使用 Tauri IPC
  useEffect(() => {
    tracing.info('Embedded mode: SSE connection skipped, using Tauri IPC');
  }, []);

  // 快捷键
  React.useEffect(() => {
    const shortcutManager = new ShortcutManager();

    shortcutManager.register({
      ...SHORTCUTS.LOCK_APP,
      action: handleLock,
    });

    shortcutManager.register({
      ...SHORTCUTS.NEW_THREAD,
      action: () => setActiveNav('chat'),
    });

    shortcutManager.register({
      ...SHORTCUTS.SEARCH,
      action: () => setActiveNav('chat'),
    });

    const handleKeyDown = (e: KeyboardEvent) => shortcutManager.handleKeyDown(e);
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);

  const handleLock = async () => {
    try {
      await sessionApi.lockApp();
      window.location.reload();
    } catch (err) {
      console.error('Failed to lock app:', err);
    }
  };

  const handleNewChat = () => {
    setSelectedThreadId(null);
    setActiveNav('chat');
  };

  const handleNavChange = (nav: NavItem) => {
    if (nav === 'settings') {
      setSettingsOpen(true);
    } else if (nav === 'routines') {
      setRoutinesOpen(true);
    } else {
      setActiveNav(nav);
    }
  };

  const renderContent = () => {
    switch (activeNav) {
      case 'chat':
        return (
          <ChatTabTauri
            selectedThreadId={selectedThreadId}
            onThreadSelect={setSelectedThreadId}
          />
        );
      case 'logs':
        return <LogsTab />;
      default:
        return null;
    }
  };

  return (
    <SidebarProvider defaultOpen>
      <AppSidebar
        activeNav={activeNav}
        onNavChange={handleNavChange}
        selectedThreadId={selectedThreadId}
        onThreadSelect={setSelectedThreadId}
        onNewChat={handleNewChat}
      />
      <SidebarInset>
        <AppHeader
          title={NAV_TITLES[activeNav]}
          onJobsClick={() => setJobsOpen(true)}
          onNotificationsClick={() => setNotificationsOpen(true)}
        />
        <div className="flex-1 overflow-hidden">{renderContent()}</div>
      </SidebarInset>

      {/* Settings Modal */}
      <SettingsModal open={settingsOpen} onOpenChange={setSettingsOpen} />

      {/* Routines Modal */}
      <RoutinesTab open={routinesOpen} onOpenChange={setRoutinesOpen} />

      {/* Jobs Panel */}
      <JobsPanel open={jobsOpen} onOpenChange={setJobsOpen} />

      {/* Notifications Panel */}
      <NotificationsPanel open={notificationsOpen} onOpenChange={setNotificationsOpen} />

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
    </SidebarProvider>
  );
}
