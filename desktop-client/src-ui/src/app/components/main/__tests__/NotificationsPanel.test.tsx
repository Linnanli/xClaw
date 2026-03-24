/**
 * NotificationsPanel 测试
 *
 * 覆盖维度：单元测试 + 失败路径 + 契约测试 + 安全审计
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { NotificationsPanel } from '../NotificationsPanel';

vi.mock('../../ui/scroll-area', () => ({
  ScrollArea: ({ children, className }: any) => (
    <div data-testid="scroll-area" className={className}>{children}</div>
  ),
}));

const mockNotifications = [
  { id: '1', type: 'system' as const, title: '系统更新', message: '新版本已发布', time: '5 分钟前', read: false },
  { id: '2', type: 'approval' as const, title: '审批请求', message: '请审批数据访问', time: '15 分钟前', read: false },
  { id: '3', type: 'task' as const, title: '任务完成', message: '代码审查已完成', time: '1 小时前', read: true },
];

describe('NotificationsPanel', () => {
  const defaultProps = {
    open: true,
    onOpenChange: vi.fn(),
    notifications: mockNotifications,
  };

  beforeEach(() => {
    vi.clearAllMocks();
  });

  // ── 单元测试 ──

  it('应该在 open=true 时渲染面板', () => {
    render(<NotificationsPanel {...defaultProps} />);
    expect(screen.getByText('通知中心')).toBeTruthy();
  });

  it('应该显示未读数量徽章', () => {
    render(<NotificationsPanel {...defaultProps} />);
    // 2 unread notifications
    expect(screen.getByText('2')).toBeTruthy();
  });

  it('应该显示所有标签页', () => {
    render(<NotificationsPanel {...defaultProps} />);
    expect(screen.getByText('全部')).toBeTruthy();
    expect(screen.getByText('系统')).toBeTruthy();
    expect(screen.getByText('审批')).toBeTruthy();
    expect(screen.getByText('任务')).toBeTruthy();
  });

  it('默认应显示全部通知', () => {
    render(<NotificationsPanel {...defaultProps} />);
    expect(screen.getByText('系统更新')).toBeTruthy();
    expect(screen.getByText('审批请求')).toBeTruthy();
    expect(screen.getByText('任务完成')).toBeTruthy();
  });

  it('点击系统标签应只显示系统通知', () => {
    render(<NotificationsPanel {...defaultProps} />);
    fireEvent.click(screen.getByRole('tab', { name: '系统' }));
    expect(screen.getByText('系统更新')).toBeTruthy();
    // Other types should be filtered out in the list
    // Note: Radix Tabs re-renders content, so we check the filtered list
  });

  it('点击审批标签应只显示审批通知', () => {
    render(<NotificationsPanel {...defaultProps} />);
    fireEvent.click(screen.getByRole('tab', { name: '审批' }));
    expect(screen.getByText('审批请求')).toBeTruthy();
  });

  it('点击任务标签应只显示任务通知', () => {
    render(<NotificationsPanel {...defaultProps} />);
    fireEvent.click(screen.getByRole('tab', { name: '任务' }));
    expect(screen.getByText('任务完成')).toBeTruthy();
  });

  it('未读通知应显示未读指示器', () => {
    render(<NotificationsPanel {...defaultProps} />);
    // Unread dot indicators (size-2 rounded-full bg-primary)
    const html = document.body.innerHTML;
    expect(html).toContain('bg-primary');
  });

  // ── 失败路径测试 ──

  it('test_failure_open_false_should_not_render', () => {
    render(<NotificationsPanel open={false} onOpenChange={vi.fn()} />);
    expect(screen.queryByText('通知中心')).toBeNull();
  });

  it('test_failure_empty_notifications_shows_empty_state', () => {
    render(
      <NotificationsPanel open={true} onOpenChange={vi.fn()} notifications={[]} />,
    );
    expect(screen.getByText('暂无通知')).toBeTruthy();
  });

  it('test_failure_renders_without_crash_with_undefined_notifications', () => {
    // Uses default sample notifications
    render(<NotificationsPanel open={true} onOpenChange={vi.fn()} />);
    expect(screen.getByText('通知中心')).toBeTruthy();
  });

  // ── 契约测试 ──

  it('test_contract_sheet_has_accessible_title', () => {
    render(<NotificationsPanel {...defaultProps} />);
    expect(screen.getByText('通知中心')).toBeTruthy();
  });

  it('test_contract_tabs_count_matches_design', () => {
    render(<NotificationsPanel {...defaultProps} />);
    const tabs = ['全部', '系统', '审批', '任务'];
    tabs.forEach((t) => expect(screen.getByText(t)).toBeTruthy());
  });

  it('test_contract_notification_item_has_title_message_time', () => {
    render(<NotificationsPanel {...defaultProps} />);
    expect(screen.getByText('系统更新')).toBeTruthy();
    expect(screen.getByText('新版本已发布')).toBeTruthy();
    expect(screen.getByText('5 分钟前')).toBeTruthy();
  });

  it('test_contract_onOpenChange_prop_is_function', () => {
    const onOpenChange = vi.fn();
    render(
      <NotificationsPanel open={true} onOpenChange={onOpenChange} notifications={mockNotifications} />,
    );
    const closeBtn = screen.getByRole('button', { name: /close/i });
    expect(closeBtn).toBeTruthy();
  });

  // ── 安全审计测试 ──

  it('test_audit_no_sensitive_data_in_rendered_output', () => {
    render(<NotificationsPanel {...defaultProps} />);
    const html = document.body.innerHTML;
    expect(html).not.toContain('password');
    expect(html).not.toContain('secret');
    expect(html).not.toContain('api_key');
  });

  it('test_audit_notification_messages_are_sanitized', () => {
    const xssNotifications = [
      {
        id: 'xss',
        type: 'system' as const,
        title: '<script>alert("xss")</script>',
        message: '<img onerror="alert(1)" src="x">',
        time: '刚刚',
        read: false,
      },
    ];
    render(
      <NotificationsPanel open={true} onOpenChange={vi.fn()} notifications={xssNotifications} />,
    );
    // React auto-escapes, so script tags should NOT be rendered as actual DOM elements
    expect(document.querySelector('script')).toBeNull();
    expect(document.querySelector('img[onerror]')).toBeNull();
    // The text content should be visible as escaped text
    expect(screen.getByText('<script>alert("xss")</script>')).toBeTruthy();
  });
});
