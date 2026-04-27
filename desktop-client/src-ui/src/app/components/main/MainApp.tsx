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
import { ChatTabTauriExperimental } from '../tabs/ChatTabTauriExperimental';
import { LogsTab } from '../tabs/LogsTab';

import { RoutinesTab } from '../tabs/RoutinesTab';
import { SettingsModal } from './SettingsModal';
import { JobsPanel } from './JobsPanel';
import { NotificationsPanel } from './NotificationsPanel';
import { DynamicWatermark } from '../common/DynamicWatermark';
import { useWatermark } from '../../hooks/useWatermark';
import { useTheme } from '../../contexts/ThemeContext';
import { sessionApi } from '../../utils/tauri';
import { ShortcutManager, SHORTCUTS } from '../../utils/shortcuts';
import { tracing } from '../../utils/tracing';
import { useChatNavigation } from '../../hooks/useChatNavigation';
import { EngineReadyProvider } from '../../hooks/useEngineReady';
import { useRunningJobs } from '../../hooks/useRunningJobs';

const NAV_TITLES: Record<NavItem, string> = {
  chat: '工作区',
  logs: '日志',
  routines: '定时任务',
  settings: '设置',
};

export function MainApp() {
  return (
    <EngineReadyProvider>
      <MainAppContent />
    </EngineReadyProvider>
  );
}

function MainAppContent() {
  const [activeNav, setActiveNav] = useState<NavItem>('chat');
  const {
    selectedThreadId,
    sidebarRefreshKey,
    pendingCommand,
    selectThread,
    clearThread,
    queueSendTextCommand,
    consumeCommand,
  } = useChatNavigation();
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [jobsOpen, setJobsOpen] = useState(false);
  const [notificationsOpen, setNotificationsOpen] = useState(false);
  const [routinesOpen, setRoutinesOpen] = useState(false);
  const { config: watermarkConfig, loading: watermarkLoading } = useWatermark();
  const { themeMode, setTheme } = useTheme();
  const runningJobs = useRunningJobs();

  // 嵌入式模式：SSE 连接跳过，使用 Tauri IPC
  useEffect(() => {
    tracing.info('Embedded mode: SSE connection skipped, using Tauri IPC');
  }, []);

  // 聊天气泡中的“查看任务”按钮通过全局事件打开任务抽屉。
  useEffect(() => {
    const handleOpenJobsPanel = () => setJobsOpen(true);
    window.addEventListener('open-jobs-panel', handleOpenJobsPanel);
    return () => window.removeEventListener('open-jobs-panel', handleOpenJobsPanel);
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
    clearThread();
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

  const handleImportFolder = (threadId: string, _path: string) => {
    selectThread(threadId);
    setActiveNav('chat');
  };

  const renderContent = () => {
    switch (activeNav) {
      case 'chat':
        // Phase 1.4: 统一走 AI-SDK 新 Runtime（旧 ChatTabTauri/TauriRuntimeProvider 已下线）。
        // - onThreadCreated: threadId === null 时由 runtime 内部调 threadApi.createThread
        //   拿到真实 id，避免 assistant-ui 临时 id 污染后端。
        // - outboundCommand: "常见问题" 等点击注入文本到 composer，由 OutboundCommandBridge
        //   通过 useComposerRuntime.setText + send() 触发，复用 Composer 的 DLP 拦截链路。
        return (
          <ChatTabTauriExperimental
            selectedThreadId={selectedThreadId}
            onThreadCreated={selectThread}
            outboundCommand={pendingCommand}
            onOutboundCommandHandled={consumeCommand}
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
        onThreadSelect={selectThread}
        onNewChat={handleNewChat}
        onImportFolder={handleImportFolder}
        refreshKey={sidebarRefreshKey}
      />
      <SidebarInset>
        <AppHeader
          title={NAV_TITLES[activeNav]}
          runningJobs={runningJobs}
          onJobsClick={() => setJobsOpen(true)}
          onNotificationsClick={() => setNotificationsOpen(true)}
          theme={themeMode}
          onThemeChange={setTheme}
        />
          <div className="min-h-0 flex-1 overflow-hidden">{renderContent()}</div>
        </SidebarInset>

        <SettingsModal open={settingsOpen} onOpenChange={setSettingsOpen} />

        <RoutinesTab
          open={routinesOpen}
          onOpenChange={setRoutinesOpen}
          onRoutineFired={() => {
            setJobsOpen(true);
          }}
        />

        <JobsPanel
          open={jobsOpen}
          onOpenChange={setJobsOpen}
          onAskJobResult={(question) => {
            setJobsOpen(false);
            setActiveNav('chat');
            queueSendTextCommand(question);
          }}
          onJobClick={(conversationId) => {
            setJobsOpen(false);
            setActiveNav('chat');
            selectThread(conversationId);
          }}
        />

        <NotificationsPanel open={notificationsOpen} onOpenChange={setNotificationsOpen} />

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
