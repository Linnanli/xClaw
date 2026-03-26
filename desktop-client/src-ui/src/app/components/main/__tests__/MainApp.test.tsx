/**
 * MainApp 单元测试
 *
 * 覆盖维度：正常路径、错误路径、契约测试、安全审计
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import { MainApp } from '../MainApp';

// Mock 子组件
vi.mock('../AppSidebar', () => ({
  AppSidebar: ({ activeNav, onNavChange, onNewChat }: any) => (
    <div data-testid="app-sidebar" data-active-nav={activeNav}>
      <button data-testid="nav-chat" onClick={() => onNavChange('chat')}>聊天</button>
      <button data-testid="nav-logs" onClick={() => onNavChange('logs')}>日志</button>
      <button data-testid="nav-settings" onClick={() => onNavChange('settings')}>设置</button>
      <button data-testid="new-chat" onClick={onNewChat}>新建</button>
    </div>
  ),
}));

vi.mock('../AppHeader', () => ({
  AppHeader: ({ title }: any) => <div data-testid="app-header">{title}</div>,
}));

vi.mock('../../tabs/ChatTabTauri', () => ({
  ChatTabTauri: () => <div data-testid="chat-tab">Chat Content</div>,
}));

vi.mock('../../tabs/LogsTab', () => ({
  LogsTab: () => <div data-testid="logs-tab">Logs Content</div>,
}));

vi.mock('../../tabs/RoutinesTab', () => ({
  RoutinesTab: () => <div data-testid="routines-tab">Routines Content</div>,
}));

vi.mock('../../common/DynamicWatermark', () => ({
  DynamicWatermark: () => null,
}));

vi.mock('../../ui/sidebar', () => ({
  SidebarProvider: ({ children }: any) => <div data-testid="sidebar-provider">{children}</div>,
  SidebarInset: ({ children }: any) => <div data-testid="sidebar-inset">{children}</div>,
}));

vi.mock('../../../hooks/useWatermark', () => ({
  useWatermark: () => ({
    config: { text: '', enabled: false, opacity: 0, fontSize: 12, rotation: 0, spacing: 0 },
    loading: false,
  }),
}));

vi.mock('../../../utils/tauri', () => ({
  sessionApi: { lockApp: vi.fn() },
}));

vi.mock('../../../utils/shortcuts', () => ({
  ShortcutManager: vi.fn().mockImplementation(() => ({
    register: vi.fn(),
    handleKeyDown: vi.fn(),
  })),
  SHORTCUTS: {
    LOCK_APP: { key: 'l', ctrl: true },
    NEW_THREAD: { key: 'n', ctrl: true },
    SEARCH: { key: 'k', ctrl: true },
  },
}));

vi.mock('../../../utils/tracing', () => ({
  tracing: { info: vi.fn(), debug: vi.fn(), warn: vi.fn() },
}));

vi.mock('../../../contexts/ThemeContext', () => ({
  useTheme: () => ({
    theme: 'light' as const,
    themeMode: 'light' as const,
    toggleTheme: vi.fn(),
    setTheme: vi.fn(),
  }),
}));

describe('MainApp', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // ===== 正常路径测试 =====

  it('应该渲染侧边栏和主内容区', () => {
    render(<MainApp />);
    expect(screen.getByTestId('sidebar-provider')).toBeInTheDocument();
    expect(screen.getByTestId('app-sidebar')).toBeInTheDocument();
    expect(screen.getByTestId('sidebar-inset')).toBeInTheDocument();
  });

  it('默认应显示聊天标签页', () => {
    render(<MainApp />);
    expect(screen.getByTestId('chat-tab')).toBeInTheDocument();
    expect(screen.getByTestId('app-header')).toHaveTextContent('聊天');
  });

  it('应该使用 SidebarProvider 包裹布局', () => {
    render(<MainApp />);
    expect(screen.getByTestId('sidebar-provider')).toBeInTheDocument();
  });

  // ===== 错误路径测试 =====

  it('test_failure_renders_without_crash', () => {
    expect(() => {
      render(<MainApp />);
    }).not.toThrow();
  });

  // ===== 契约测试 =====

  it('test_contract_sidebar_receives_correct_default_nav', () => {
    render(<MainApp />);
    expect(screen.getByTestId('app-sidebar')).toHaveAttribute('data-active-nav', 'chat');
  });

  it('test_contract_header_shows_correct_title_for_default_nav', () => {
    render(<MainApp />);
    expect(screen.getByTestId('app-header')).toHaveTextContent('聊天');
  });

  // ===== 安全审计测试 =====

  it('test_audit_no_sensitive_data_in_rendered_output', () => {
    const { container } = render(<MainApp />);
    const html = container.innerHTML;
    expect(html).not.toMatch(/password/i);
    expect(html).not.toMatch(/session[_-]?id/i);
    expect(html).not.toMatch(/api[_-]?key/i);
  });
});
