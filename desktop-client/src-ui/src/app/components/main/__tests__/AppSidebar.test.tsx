/**
 * AppSidebar 单元测试
 *
 * 覆盖维度：
 * - 正常路径：渲染、导航切换、新建聊天
 * - 错误路径：线程加载失败
 * - 契约测试：props 接口一致性
 * - 安全审计：无敏感信息泄露
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { AppSidebar, type NavItem } from '../AppSidebar';

// Mock shadcn/ui sidebar 组件
vi.mock('../../ui/sidebar', () => ({
  Sidebar: ({ children, ...props }: any) => <div data-testid="sidebar" {...props}>{children}</div>,
  SidebarContent: ({ children }: any) => <div data-testid="sidebar-content">{children}</div>,
  SidebarFooter: ({ children }: any) => <div data-testid="sidebar-footer">{children}</div>,
  SidebarGroup: ({ children }: any) => <div>{children}</div>,
  SidebarGroupContent: ({ children }: any) => <div>{children}</div>,
  SidebarGroupLabel: ({ children }: any) => <div>{children}</div>,
  SidebarHeader: ({ children }: any) => <div data-testid="sidebar-header">{children}</div>,
  SidebarMenu: ({ children }: any) => <ul>{children}</ul>,
  SidebarMenuButton: ({ children, onClick, isActive, ...props }: any) => (
    <button onClick={onClick} data-active={isActive} {...props}>{children}</button>
  ),
  SidebarMenuItem: ({ children }: any) => <li>{children}</li>,
  SidebarSeparator: () => <hr />,
}));

vi.mock('../../ui/button', () => ({
  Button: ({ children, onClick, ...props }: any) => (
    <button onClick={onClick} {...props}>{children}</button>
  ),
}));

vi.mock('../../ui/scroll-area', () => ({
  ScrollArea: ({ children }: any) => <div>{children}</div>,
}));

vi.mock('../../ui/tooltip', () => ({
  Tooltip: ({ children }: any) => <div>{children}</div>,
  TooltipContent: ({ children }: any) => <div>{children}</div>,
  TooltipTrigger: ({ children }: any) => <div>{children}</div>,
}));

vi.mock('../../../utils/tauri', () => ({
  threadApi: {
    getThreads: vi.fn().mockResolvedValue([]),
    createThread: vi.fn().mockResolvedValue({ id: 'new-thread-id', title: '新对话', created_at: new Date().toISOString(), updated_at: new Date().toISOString() }),
  },
  invokeTauri: vi.fn().mockResolvedValue(''),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn().mockResolvedValue(null),
}));

const defaultProps = {
  activeNav: 'chat' as NavItem,
  onNavChange: vi.fn(),
  selectedThreadId: null,
  onThreadSelect: vi.fn(),
  onNewChat: vi.fn(),
  onImportFolder: vi.fn(),
};

describe('AppSidebar', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // ===== 正常路径测试 =====

  it('应该正确渲染侧边栏结构', () => {
    render(<AppSidebar {...defaultProps} />);
    expect(screen.getByTestId('sidebar')).toBeInTheDocument();
    expect(screen.getByTestId('sidebar-header')).toBeInTheDocument();
    expect(screen.getByTestId('sidebar-content')).toBeInTheDocument();
    expect(screen.getByTestId('sidebar-footer')).toBeInTheDocument();
  });

  it('应该显示 X-Claw 品牌标识', () => {
    render(<AppSidebar {...defaultProps} />);
    expect(screen.getByText('X-Claw')).toBeInTheDocument();
  });

  it('应该显示所有底部导航项', () => {
    render(<AppSidebar {...defaultProps} />);
    expect(screen.getByText('工作区')).toBeInTheDocument();
    expect(screen.getByText('日志')).toBeInTheDocument();
    expect(screen.getByText('定时任务')).toBeInTheDocument();
    expect(screen.getByText('设置')).toBeInTheDocument();
  });

  it('点击新建对话按钮应触发 onNewChat', () => {
    render(<AppSidebar {...defaultProps} />);
    const newChatBtn = screen.getByLabelText('新建对话');
    fireEvent.click(newChatBtn);
    expect(defaultProps.onNewChat).toHaveBeenCalledTimes(1);
  });

  it('点击导航项应触发 onNavChange', () => {
    render(<AppSidebar {...defaultProps} />);
    fireEvent.click(screen.getByText('日志'));
    expect(defaultProps.onNavChange).toHaveBeenCalledWith('logs');
  });

  it('点击设置应触发 onNavChange("settings")', () => {
    render(<AppSidebar {...defaultProps} />);
    fireEvent.click(screen.getByText('设置'));
    expect(defaultProps.onNavChange).toHaveBeenCalledWith('settings');
  });

  it('应该显示用户头像区域', () => {
    render(<AppSidebar {...defaultProps} />);
    expect(screen.getByText('用户')).toBeInTheDocument();
    expect(screen.getByText('U')).toBeInTheDocument();
  });

  // ===== 错误路径测试 =====

  it('test_failure_thread_load_error_should_not_crash', async () => {
    const { threadApi } = await import('../../../utils/tauri');
    vi.mocked(threadApi.getThreads).mockRejectedValueOnce(new Error('Network error'));

    // 不应该抛出异常
    expect(() => {
      render(<AppSidebar {...defaultProps} />);
    }).not.toThrow();
  });

  // ===== 契约测试 =====

  it('test_contract_nav_items_match_expected_values', () => {
    const validNavItems: NavItem[] = ['chat', 'logs', 'routines', 'settings'];
    validNavItems.forEach((nav) => {
      const onNavChange = vi.fn();
      render(<AppSidebar {...defaultProps} onNavChange={onNavChange} activeNav={nav} />);
    });
  });

  it('test_contract_props_interface_accepts_null_thread_id', () => {
    expect(() => {
      render(<AppSidebar {...defaultProps} selectedThreadId={null} />);
    }).not.toThrow();
  });

  it('test_contract_props_interface_accepts_string_thread_id', () => {
    expect(() => {
      render(<AppSidebar {...defaultProps} selectedThreadId="thread-123" />);
    }).not.toThrow();
  });

  // ===== 安全审计测试 =====

  it('test_audit_no_sensitive_data_in_rendered_output', () => {
    const { container } = render(<AppSidebar {...defaultProps} />);
    const html = container.innerHTML;

    // 不应包含任何 token、密码、密钥等敏感信息
    expect(html).not.toMatch(/password/i);
    expect(html).not.toMatch(/token/i);
    expect(html).not.toMatch(/secret/i);
    expect(html).not.toMatch(/api[_-]?key/i);
  });
});
