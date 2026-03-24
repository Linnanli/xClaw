/**
 * AppHeader 单元测试
 *
 * 覆盖维度：正常路径、错误路径、契约测试、安全审计
 */

import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { AppHeader } from '../AppHeader';

describe('AppHeader', () => {
  // ===== 正常路径测试 =====

  it('应该渲染标题', () => {
    render(<AppHeader title="聊天" />);
    expect(screen.getByText('聊天')).toBeInTheDocument();
  });

  it('应该渲染任务按钮', () => {
    render(<AppHeader title="聊天" />);
    expect(screen.getByText('任务')).toBeInTheDocument();
  });

  it('应该渲染通知按钮', () => {
    render(<AppHeader title="聊天" />);
    expect(screen.getByLabelText('通知')).toBeInTheDocument();
  });

  it('有运行中任务时应显示计数', () => {
    render(<AppHeader title="聊天" runningJobs={3} />);
    expect(screen.getByText('3')).toBeInTheDocument();
  });

  it('无运行中任务时不显示计数', () => {
    render(<AppHeader title="聊天" runningJobs={0} />);
    expect(screen.queryByText('0')).not.toBeInTheDocument();
  });

  it('有未读通知时应显示红点', () => {
    const { container } = render(<AppHeader title="聊天" unreadCount={5} />);
    // 设计稿：8x8 红点指示器（无数字）
    const badge = container.querySelector('.bg-\\[\\#D94040\\]');
    expect(badge).toBeInTheDocument();
  });

  it('无未读通知时不显示红点', () => {
    const { container } = render(<AppHeader title="聊天" unreadCount={0} />);
    const badge = container.querySelector('.bg-\\[\\#D94040\\]');
    expect(badge).not.toBeInTheDocument();
  });

  it('点击任务按钮应触发回调', () => {
    const onJobsClick = vi.fn();
    render(<AppHeader title="聊天" onJobsClick={onJobsClick} />);
    fireEvent.click(screen.getByText('任务'));
    expect(onJobsClick).toHaveBeenCalledTimes(1);
  });

  it('点击通知按钮应触发回调', () => {
    const onNotificationsClick = vi.fn();
    render(<AppHeader title="聊天" onNotificationsClick={onNotificationsClick} />);
    fireEvent.click(screen.getByLabelText('通知'));
    expect(onNotificationsClick).toHaveBeenCalledTimes(1);
  });

  // ===== 错误路径测试 =====

  it('test_failure_no_callbacks_should_not_crash', () => {
    expect(() => {
      render(<AppHeader title="聊天" />);
      fireEvent.click(screen.getByText('任务'));
      fireEvent.click(screen.getByLabelText('通知'));
    }).not.toThrow();
  });

  it('test_failure_empty_title_renders_without_error', () => {
    expect(() => {
      render(<AppHeader title="" />);
    }).not.toThrow();
  });

  // ===== 契约测试 =====

  it('test_contract_accepts_optional_props', () => {
    expect(() => {
      render(<AppHeader title="测试" />);
    }).not.toThrow();
  });

  it('test_contract_accepts_all_props', () => {
    expect(() => {
      render(
        <AppHeader
          title="聊天"
          runningJobs={2}
          unreadCount={5}
          onJobsClick={() => {}}
          onNotificationsClick={() => {}}
          className="custom-class"
        />,
      );
    }).not.toThrow();
  });

  // ===== 安全审计测试 =====

  it('test_audit_no_sensitive_data_in_output', () => {
    const { container } = render(
      <AppHeader title="聊天" runningJobs={3} unreadCount={5} />,
    );
    const html = container.innerHTML;
    expect(html).not.toMatch(/password/i);
    expect(html).not.toMatch(/token/i);
    expect(html).not.toMatch(/secret/i);
  });
});
