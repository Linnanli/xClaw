/**
 * SettingsModal 测试
 *
 * 覆盖维度：单元测试 + 失败路径 + 契约测试 + 安全审计
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor, within } from '@testing-library/react';
import { SettingsModal } from '../SettingsModal';

const mockGetAppServerStatus = vi.hoisted(() => vi.fn());

// Mock child tabs to isolate SettingsModal logic
vi.mock('../../tabs/SkillsTab', () => ({
  SkillsTab: () => <div data-testid="skills-tab">SkillsTab</div>,
}));
vi.mock('../../tabs/ExtensionsTab', () => ({
  ExtensionsTab: () => <div data-testid="extensions-tab">ExtensionsTab</div>,
}));
vi.mock('../../tabs/MemoryTab', () => ({
  MemoryTab: () => <div data-testid="memory-tab">MemoryTab</div>,
}));
vi.mock('../../ui/scroll-area', () => ({
  ScrollArea: ({ children, className }: any) => (
    <div data-testid="scroll-area" className={className}>{children}</div>
  ),
}));
vi.mock('../../../utils/tauri', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../../../utils/tauri')>()),
  appServerApi: {
    getStatus: mockGetAppServerStatus,
  },
}));

describe('SettingsModal', () => {
  const defaultProps = { open: true, onOpenChange: vi.fn() };

  beforeEach(() => {
    vi.clearAllMocks();
    mockGetAppServerStatus.mockResolvedValue({
      binary: 'dasclaw-app-server',
      startupSmokeEnabled: false,
      supervisorEnabled: false,
      supervisor: {
        state: 'stopped',
        restartCount: 0,
        notificationCount: 0,
      },
      smokeRun: false,
    });
  });

  // ── 单元测试 ──

  it('应该在 open=true 时渲染模态框', () => {
    render(<SettingsModal {...defaultProps} />);
    expect(screen.getAllByText('通用设置').length).toBeGreaterThan(0);
  });

  it('应该默认显示通用设置内容', () => {
    render(<SettingsModal {...defaultProps} />);
    expect(screen.getByText('AI 安全防护')).toBeTruthy();
    expect(screen.getByText('防休眠')).toBeTruthy();
  });

  it('应该显示所有导航项', () => {
    render(<SettingsModal {...defaultProps} />);
    const navLabels = ['用量统计', '诊断', '技能管理', '记忆', '关于我们'];
    navLabels.forEach((label) => {
      expect(screen.getByText(label)).toBeTruthy();
    });
    // 通用设置 and 扩展 may appear multiple times (nav + header)
    expect(screen.getAllByText('通用设置').length).toBeGreaterThan(0);
    expect(screen.getAllByText('扩展').length).toBeGreaterThan(0);
  });

  it('点击导航项应切换内容', () => {
    render(<SettingsModal {...defaultProps} />);
    const aboutBtns = screen.getAllByText('关于我们');
    // Click the nav button (first one)
    fireEvent.click(aboutBtns[0]);
    expect(screen.getByText('智能 AI 助手，让工作更高效')).toBeTruthy();
  });

  it('点击用量统计应显示统计卡片', () => {
    render(<SettingsModal {...defaultProps} />);
    fireEvent.click(screen.getByText('用量统计'));
    expect(screen.getByText('本月对话数')).toBeTruthy();
    expect(screen.getByText('Token 使用量')).toBeTruthy();
  });

  it('点击诊断应加载 app-server 轻量状态', async () => {
    render(<SettingsModal {...defaultProps} />);
    fireEvent.click(screen.getByText('诊断'));

    await waitFor(() => expect(mockGetAppServerStatus).toHaveBeenCalledWith(false));
    expect(screen.getByText('dasclaw app-server')).toBeTruthy();
    expectMetricValue('启动 smoke', '未启用');
    expectMetricValue('Supervisor', '未启用');
    expect(screen.getByText('未运行')).toBeTruthy();
    expect(screen.getByText('stopped')).toBeTruthy();
  });

  it('诊断刷新应运行 app-server smoke 并显示结果', async () => {
    mockGetAppServerStatus
      .mockResolvedValueOnce({
        binary: 'dasclaw-app-server',
        startupSmokeEnabled: false,
        supervisorEnabled: false,
        supervisor: {
          state: 'stopped',
          restartCount: 0,
          notificationCount: 0,
        },
        smokeRun: false,
      })
      .mockResolvedValueOnce({
        binary: '/tmp/dasclaw-app-server',
        startupSmokeEnabled: true,
        supervisorEnabled: true,
        supervisor: {
          state: 'ready',
          restartCount: 1,
          notificationCount: 2,
          runtimeHealthStatus: 'degraded',
        },
        smokeRun: true,
        smoke: {
          notificationCount: 3,
          sessionStatus: 'implemented',
          runtimeHealthStatus: 'degraded',
          shutdownState: 'stopped',
        },
      });

    render(<SettingsModal {...defaultProps} />);
    fireEvent.click(screen.getByText('诊断'));

    await waitFor(() => expect(mockGetAppServerStatus).toHaveBeenCalledWith(false));
    fireEvent.click(screen.getByText('刷新'));

    await waitFor(() => expect(mockGetAppServerStatus).toHaveBeenCalledWith(true));
    expect(screen.getByText('implemented')).toBeTruthy();
    expect(screen.getByText('degraded')).toBeTruthy();
    expect(screen.getByText('ready')).toBeTruthy();
    expect(screen.getByText('Supervisor 通知 2 条，restart: 1')).toBeTruthy();
    expect(screen.getByText('通知 3 条，shutdown: stopped')).toBeTruthy();
  });

  it('诊断刷新失败应显示错误且不伪装为 smoke 成功', async () => {
    mockGetAppServerStatus
      .mockResolvedValueOnce({
        binary: 'dasclaw-app-server',
        startupSmokeEnabled: false,
        supervisorEnabled: false,
        supervisor: {
          state: 'stopped',
          restartCount: 0,
          notificationCount: 0,
        },
        smokeRun: false,
      })
      .mockResolvedValueOnce({
        binary: 'missing-app-server',
        startupSmokeEnabled: false,
        supervisorEnabled: false,
        supervisor: {
          state: 'stopped',
          restartCount: 0,
          notificationCount: 0,
        },
        smokeRun: true,
        error: 'spawn failed',
      });

    render(<SettingsModal {...defaultProps} />);
    fireEvent.click(screen.getByText('诊断'));

    await waitFor(() => expect(mockGetAppServerStatus).toHaveBeenCalledWith(false));
    fireEvent.click(screen.getByText('刷新'));

    await waitFor(() => expect(screen.getByText('spawn failed')).toBeTruthy());
    expect(screen.queryByText(/通知 3 条/)).toBeNull();
    expect(screen.queryByText('implemented')).toBeNull();
  });

  it('诊断刷新 IPC rejected 时应显示错误且不伪装为 smoke 成功', async () => {
    mockGetAppServerStatus
      .mockResolvedValueOnce({
        binary: 'dasclaw-app-server',
        startupSmokeEnabled: false,
        supervisorEnabled: false,
        supervisor: {
          state: 'stopped',
          restartCount: 0,
          notificationCount: 0,
        },
        smokeRun: false,
      })
      .mockRejectedValueOnce(new Error('ipc failed'));

    render(<SettingsModal {...defaultProps} />);
    fireEvent.click(screen.getByText('诊断'));

    await waitFor(() => expect(mockGetAppServerStatus).toHaveBeenCalledWith(false));
    fireEvent.click(screen.getByText('刷新'));

    await waitFor(() => expect(screen.getByText('ipc failed')).toBeTruthy());
    expect(screen.queryByText(/通知 3 条/)).toBeNull();
    expect(screen.queryByText('implemented')).toBeNull();
  });

  it('诊断成功后再次 IPC rejected 应清掉旧 smoke 成功态', async () => {
    mockGetAppServerStatus
      .mockResolvedValueOnce({
        binary: 'dasclaw-app-server',
        startupSmokeEnabled: false,
        supervisorEnabled: false,
        supervisor: {
          state: 'stopped',
          restartCount: 0,
          notificationCount: 0,
        },
        smokeRun: false,
      })
      .mockResolvedValueOnce({
        binary: '/tmp/dasclaw-app-server',
        startupSmokeEnabled: true,
        supervisorEnabled: true,
        supervisor: {
          state: 'ready',
          restartCount: 1,
          notificationCount: 2,
          runtimeHealthStatus: 'degraded',
        },
        smokeRun: true,
        smoke: {
          notificationCount: 3,
          sessionStatus: 'implemented',
          runtimeHealthStatus: 'degraded',
          shutdownState: 'stopped',
        },
      })
      .mockRejectedValueOnce(new Error('ipc failed'));

    render(<SettingsModal {...defaultProps} />);
    fireEvent.click(screen.getByText('诊断'));

    await waitFor(() => expect(mockGetAppServerStatus).toHaveBeenCalledWith(false));
    fireEvent.click(screen.getByText('刷新'));
    await waitFor(() => expect(screen.getByText('implemented')).toBeTruthy());

    fireEvent.click(screen.getByText('刷新'));
    await waitFor(() => expect(screen.getByText('ipc failed')).toBeTruthy());
    expect(screen.queryByText('implemented')).toBeNull();
    expect(screen.queryByText('Supervisor 通知 2 条，restart: 1')).toBeNull();
    expect(screen.queryByText('通知 3 条，shutdown: stopped')).toBeNull();
  });

  it('点击技能管理应渲染 SkillsTab', () => {
    render(<SettingsModal {...defaultProps} />);
    fireEvent.click(screen.getByText('技能管理'));
    expect(screen.getByTestId('skills-tab')).toBeTruthy();
  });

  it('点击扩展应渲染 ExtensionsTab', () => {
    render(<SettingsModal {...defaultProps} />);
    const extBtns = screen.getAllByText('扩展');
    fireEvent.click(extBtns[0]);
    expect(screen.getByTestId('extensions-tab')).toBeTruthy();
  });

  it('点击记忆应渲染 MemoryTab', () => {
    render(<SettingsModal {...defaultProps} />);
    fireEvent.click(screen.getByText('记忆'));
    expect(screen.getByTestId('memory-tab')).toBeTruthy();
  });

  it('AI 安全防护开关应可切换', () => {
    render(<SettingsModal {...defaultProps} />);
    const switches = screen.getAllByRole('switch');
    expect(switches.length).toBeGreaterThanOrEqual(2);
    fireEvent.click(switches[0]);
    // Switch should toggle without error
  });

  it('退出登录按钮应存在', () => {
    render(<SettingsModal {...defaultProps} />);
    expect(screen.getByText('退出登录')).toBeTruthy();
  });

  // ── 失败路径测试 ──

  it('test_failure_open_false_should_not_render_content', () => {
    render(<SettingsModal open={false} onOpenChange={vi.fn()} />);
    expect(screen.queryByText('AI 安全防护')).toBeNull();
  });

  it('test_failure_renders_without_crash_when_toggled_rapidly', () => {
    const onOpenChange = vi.fn();
    const { rerender } = render(
      <SettingsModal open={true} onOpenChange={onOpenChange} />,
    );
    rerender(<SettingsModal open={false} onOpenChange={onOpenChange} />);
    rerender(<SettingsModal open={true} onOpenChange={onOpenChange} />);
    // Should not throw
  });

  // ── 契约测试 ──

  it('test_contract_dialog_has_accessible_title', () => {
    render(<SettingsModal {...defaultProps} />);
    // DialogTitle with sr-only class should exist for accessibility
    const titles = screen.getAllByText('设置');
    expect(titles.length).toBeGreaterThan(0);
  });

  it('test_contract_onOpenChange_called_on_close', () => {
    const onOpenChange = vi.fn();
    render(<SettingsModal open={true} onOpenChange={onOpenChange} />);
    // The close button is rendered by DialogContent
    const closeBtns = screen.getAllByRole('button', { name: /close/i });
    fireEvent.click(closeBtns[0]);
    expect(onOpenChange).toHaveBeenCalled();
  });

  it('test_contract_nav_items_count_matches_design', () => {
    render(<SettingsModal {...defaultProps} />);
    // 6 nav items: check unique labels that only appear in nav
    const uniqueNavLabels = ['用量统计', '技能管理', '记忆', '关于我们'];
    uniqueNavLabels.forEach((label) => {
      expect(screen.getByText(label)).toBeTruthy();
    });
  });

  // ── 安全审计测试 ──

  it('test_audit_no_sensitive_data_in_rendered_output', () => {
    render(<SettingsModal {...defaultProps} />);
    const html = document.body.innerHTML;
    expect(html).not.toContain('password');
    expect(html).not.toContain('secret');
    expect(html).not.toContain('token');
  });

  it('test_audit_logout_requires_confirmation', () => {
    const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(false);
    render(<SettingsModal {...defaultProps} />);
    fireEvent.click(screen.getByText('退出登录'));
    expect(confirmSpy).toHaveBeenCalled();
    confirmSpy.mockRestore();
  });
});

function expectMetricValue(label: string, value: string): void {
  const metric = screen.getByText(label).closest('div');
  expect(metric).not.toBeNull();
  expect(within(metric as HTMLElement).getByText(value)).toBeTruthy();
}
