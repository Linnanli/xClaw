/**
 * RoutinesTab 测试
 *
 * 覆盖维度：单元测试（正常路径 + 错误路径）、契约测试、安全审计测试
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { RoutinesTab } from '../RoutinesTab';

// Mock tauri event（useEngineReady 内部使用）
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

// Mock tauri APIs
vi.mock('../../../utils/tauri', () => ({
  routineApi: {
    getRoutines: vi.fn().mockResolvedValue([
      {
        id: 'r1',
        name: '每日工作汇报',
        description: '每天早上总结昨日工作进展',
        trigger: 'time',
        triggerValue: '0 9 * * *',
        status: 'active',
      },
      {
        id: 'r2',
        name: 'GitHub Issue 分析',
        description: '新 issue 触发时自动分析',
        trigger: 'event',
        triggerValue: '',
        status: 'active',
      },
      {
        id: 'r3',
        name: '消息监控',
        description: '匹配特定消息模式',
        trigger: 'manual',
        triggerValue: '',
        status: 'paused',
      },
    ]),
    createRoutine: vi.fn().mockResolvedValue({ id: 'new-1' }),
    deleteRoutine: vi.fn().mockResolvedValue(undefined),
    triggerRoutine: vi.fn().mockResolvedValue(undefined),
  },
  routineExtendedApi: {
    pauseRoutine: vi.fn().mockResolvedValue(undefined),
    enableRoutine: vi.fn().mockResolvedValue(undefined),
    getRoutineRuns: vi.fn().mockResolvedValue({ runs: [] }),
  },
}));

describe('RoutinesTab', () => {
  const mockOnOpenChange = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
  });

  // ── 单元测试（正常路径）──

  it('应该渲染模态框结构', async () => {
    render(<RoutinesTab open onOpenChange={mockOnOpenChange} />);
    await waitFor(() => {
      // 左侧导航标题
      expect(screen.getAllByText('定时任务').length).toBeGreaterThan(0);
    });
  });

  it('应该显示左侧导航项', async () => {
    render(<RoutinesTab open onOpenChange={mockOnOpenChange} />);
    await waitFor(() => {
      expect(screen.getAllByText('全部任务').length).toBeGreaterThan(0);
      expect(screen.getByText('已启用')).toBeInTheDocument();
      expect(screen.getByText('已禁用')).toBeInTheDocument();
    });
  });

  it('应该加载并显示任务列表', async () => {
    render(<RoutinesTab open onOpenChange={mockOnOpenChange} />);
    await waitFor(() => {
      expect(screen.getByText('每日工作汇报')).toBeInTheDocument();
      expect(screen.getByText('GitHub Issue 分析')).toBeInTheDocument();
      expect(screen.getByText('消息监控')).toBeInTheDocument();
    });
  });

  it('应该显示任务描述', async () => {
    render(<RoutinesTab open onOpenChange={mockOnOpenChange} />);
    await waitFor(() => {
      expect(screen.getByText('每天早上总结昨日工作进展')).toBeInTheDocument();
    });
  });

  it('应该显示新建按钮', async () => {
    render(<RoutinesTab open onOpenChange={mockOnOpenChange} />);
    await waitFor(() => {
      expect(screen.getByText('新建定时任务')).toBeInTheDocument();
    });
  });

  it('点击已启用应过滤任务', async () => {
    render(<RoutinesTab open onOpenChange={mockOnOpenChange} />);
    await waitFor(() => {
      expect(screen.getByText('每日工作汇报')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByText('已启用'));
    await waitFor(() => {
      expect(screen.getByText('每日工作汇报')).toBeInTheDocument();
      expect(screen.getByText('GitHub Issue 分析')).toBeInTheDocument();
      expect(screen.queryByText('消息监控')).not.toBeInTheDocument();
    });
  });

  it('点击已禁用应过滤任务', async () => {
    render(<RoutinesTab open onOpenChange={mockOnOpenChange} />);
    await waitFor(() => {
      expect(screen.getByText('消息监控')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByText('已禁用'));
    await waitFor(() => {
      expect(screen.getByText('消息监控')).toBeInTheDocument();
      expect(screen.queryByText('每日工作汇报')).not.toBeInTheDocument();
    });
  });

  it('点击新建应显示创建表单', async () => {
    render(<RoutinesTab open onOpenChange={mockOnOpenChange} />);
    await waitFor(() => {
      expect(screen.getByText('新建定时任务')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByText('新建定时任务'));
    await waitFor(() => {
      expect(screen.getByPlaceholderText('输入任务名称')).toBeInTheDocument();
      expect(screen.getByPlaceholderText('输入任务描述')).toBeInTheDocument();
    });
  });

  it('应该显示运行按钮', async () => {
    render(<RoutinesTab open onOpenChange={mockOnOpenChange} />);
    await waitFor(() => {
      const runButtons = screen.getAllByText('立即运行');
      expect(runButtons.length).toBeGreaterThan(0);
    });
  });

  // ── 单元测试（错误路径）──

  it('test_failure_api_error_shows_empty_state', async () => {
    const { routineApi } = await import('../../../utils/tauri');
    vi.mocked(routineApi.getRoutines).mockRejectedValueOnce(new Error('Network error'));
    render(<RoutinesTab open onOpenChange={mockOnOpenChange} />);
    await waitFor(() => {
      // API 错误时显示错误信息（不再显示"暂无定时任务"）
      expect(screen.getByText(/加载失败/)).toBeInTheDocument();
    });
  });

  it('test_failure_closed_modal_does_not_render_content', () => {
    render(<RoutinesTab open={false} onOpenChange={mockOnOpenChange} />);
    expect(screen.queryByText('全部任务')).not.toBeInTheDocument();
  });

  // ── 契约测试 ──

  it('test_contract_nav_filter_keys_match_expected', async () => {
    render(<RoutinesTab open onOpenChange={mockOnOpenChange} />);
    await waitFor(() => {
      // 设计稿定义了 3 个导航项
      expect(screen.getAllByText('全部任务').length).toBeGreaterThan(0);
      expect(screen.getByText('已启用')).toBeInTheDocument();
      expect(screen.getByText('已禁用')).toBeInTheDocument();
    });
  });

  it('test_contract_card_structure_matches_design', async () => {
    render(<RoutinesTab open onOpenChange={mockOnOpenChange} />);
    await waitFor(() => {
      // 每张卡片应有：名称、描述、立即运行按钮
      expect(screen.getByText('每日工作汇报')).toBeInTheDocument();
      expect(screen.getByText('每天早上总结昨日工作进展')).toBeInTheDocument();
      expect(screen.getAllByText('立即运行').length).toBe(3);
    });
  });

  it('test_contract_modal_dimensions_class', async () => {
    render(<RoutinesTab open onOpenChange={mockOnOpenChange} />);
    await waitFor(() => {
      const dialog = document.querySelector('[class*="w-[960px]"]');
      expect(dialog).toBeTruthy();
    });
  });

  // ── 安全审计测试 ──

  it('test_audit_no_sensitive_data_in_rendered_output', async () => {
    render(<RoutinesTab open onOpenChange={mockOnOpenChange} />);
    await waitFor(() => {
      const html = document.body.innerHTML;
      expect(html).not.toContain('password');
      expect(html).not.toContain('secret');
      expect(html).not.toContain('token');
    });
  });

  it('test_audit_create_form_validates_empty_input', async () => {
    render(<RoutinesTab open onOpenChange={mockOnOpenChange} />);
    await waitFor(() => {
      expect(screen.getByText('新建定时任务')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByText('新建定时任务'));
    await waitFor(() => {
      const createBtn = screen.getAllByText('创建').find(
        (el) => el.tagName === 'BUTTON' && el.closest('[class*="shadow-xl"]'),
      );
      expect(createBtn).toBeTruthy();
      // 创建按钮应该被禁用（名称为空）
      expect(createBtn).toBeDisabled();
    });
  });
});
