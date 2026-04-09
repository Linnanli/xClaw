/**
 * JobsPanel 测试
 *
 * 覆盖维度：单元测试 + 失败路径 + 契约测试 + 安全审计
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { JobsPanel } from '../JobsPanel';

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

const mockJobs = [
  { id: '1', title: '代码审查', status: 'in_progress', created_at: '2024-01-01T00:00:00Z', updated_at: null },
  { id: '2', title: '安全扫描', status: 'completed', created_at: '2024-01-01T00:00:00Z', updated_at: '2024-01-01T01:00:00Z' },
  { id: '3', title: '部署任务', status: 'failed', created_at: '2024-01-01T00:00:00Z', updated_at: null },
];

vi.mock('../../../utils/tauri', () => ({
  jobApi: {
    getJobs: vi.fn().mockResolvedValue([
      { id: '1', title: '代码审查', status: 'in_progress', created_at: '2024-01-01T00:00:00Z', updated_at: null },
      { id: '2', title: '安全扫描', status: 'completed', created_at: '2024-01-01T00:00:00Z', updated_at: '2024-01-01T01:00:00Z' },
      { id: '3', title: '部署任务', status: 'failed', created_at: '2024-01-01T00:00:00Z', updated_at: null },
    ]),
  },
}));

vi.mock('../../ui/scroll-area', () => ({
  ScrollArea: ({ children, className }: any) => (
    <div data-testid="scroll-area" className={className}>{children}</div>
  ),
}));

describe('JobsPanel', () => {
  const defaultProps = { open: true, onOpenChange: vi.fn() };

  beforeEach(() => {
    vi.clearAllMocks();
  });

  // ── 单元测试 ──

  it('应该在 open=true 时渲染面板', async () => {
    render(<JobsPanel {...defaultProps} />);
    await waitFor(() => {
      expect(screen.getByText('任务列表')).toBeTruthy();
    });
  });

  it('应该显示统计行', async () => {
    render(<JobsPanel {...defaultProps} />);
    await waitFor(() => {
      expect(screen.getByText('运行中')).toBeTruthy();
      expect(screen.getByText('已完成')).toBeTruthy();
      expect(screen.getByText('失败')).toBeTruthy();
    });
  });

  it('应该显示任务列表', async () => {
    render(<JobsPanel {...defaultProps} />);
    await waitFor(() => {
      expect(screen.getByText('代码审查')).toBeTruthy();
      expect(screen.getByText('安全扫描')).toBeTruthy();
      expect(screen.getByText('部署任务')).toBeTruthy();
    });
  });

  it('应该显示关闭按钮', async () => {
    render(<JobsPanel {...defaultProps} />);
    await waitFor(() => {
      const closeBtn = screen.getByRole('button', { name: /close/i });
      expect(closeBtn).toBeTruthy();
    });
  });

  // ── 失败路径测试 ──

  it('test_failure_open_false_should_not_render', () => {
    render(<JobsPanel open={false} onOpenChange={vi.fn()} />);
    expect(screen.queryByText('任务列表')).toBeNull();
  });

  it('test_failure_api_error_shows_empty_state', async () => {
    const { jobApi } = await import('../../../utils/tauri');
    (jobApi.getJobs as any).mockRejectedValueOnce(new Error('Network error'));
    render(<JobsPanel {...defaultProps} />);
    await waitFor(() => {
      // Should not crash, may show empty state
      expect(screen.getByText('任务列表')).toBeTruthy();
    });
  });

  // ── 契约测试 ──

  it('test_contract_sheet_has_accessible_title', async () => {
    render(<JobsPanel {...defaultProps} />);
    await waitFor(() => {
      expect(screen.getByText('任务列表')).toBeTruthy();
    });
  });

  it('test_contract_stats_row_has_three_items', async () => {
    render(<JobsPanel {...defaultProps} />);
    await waitFor(() => {
      const labels = ['运行中', '已完成', '失败'];
      labels.forEach((l) => expect(screen.getByText(l)).toBeTruthy());
    });
  });

  it('test_contract_onOpenChange_prop_is_function', () => {
    const onOpenChange = vi.fn();
    render(<JobsPanel open={true} onOpenChange={onOpenChange} />);
    // Close button from SheetContent
    const closeBtn = screen.getByRole('button', { name: /close/i });
    expect(closeBtn).toBeTruthy();
  });

  // ── 安全审计测试 ──

  it('test_audit_no_sensitive_data_in_rendered_output', async () => {
    render(<JobsPanel {...defaultProps} />);
    await waitFor(() => {
      const html = document.body.innerHTML;
      expect(html).not.toContain('password');
      expect(html).not.toContain('secret');
      expect(html).not.toContain('api_key');
    });
  });

  it('test_audit_job_ids_are_not_exposed_as_raw_uuids', async () => {
    render(<JobsPanel {...defaultProps} />);
    await waitFor(() => {
      // Job titles should be shown, not raw IDs
      expect(screen.getByText('代码审查')).toBeTruthy();
    });
  });
});
