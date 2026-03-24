/**
 * LogsTab 测试
 *
 * 覆盖维度：单元测试（正常路径 + 错误路径）、契约测试、安全审计测试
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { LogsTab } from '../LogsTab';

// Mock tauri APIs
vi.mock('../../../utils/tauri', () => ({
  logApi: {
    getLogs: vi.fn().mockResolvedValue([
      { timestamp: '2024-01-01T10:00:00Z', level: 'info', module: 'auth', message: '用户登录成功', context: null },
      { timestamp: '2024-01-01T10:01:00Z', level: 'warn', module: 'dlp', message: 'DLP 规则匹配', context: null },
      { timestamp: '2024-01-01T10:02:00Z', level: 'error', module: 'network', message: '连接超时', context: { code: 408 } },
    ]),
    searchLogs: vi.fn().mockResolvedValue([]),
    filterLogs: vi.fn().mockResolvedValue([]),
    exportLogs: vi.fn().mockResolvedValue('[]'),
  },
  logClearApi: {
    clearLogs: vi.fn().mockResolvedValue(undefined),
  },
}));

describe('LogsTab', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // ── 单元测试（正常路径）──

  it('应该渲染日志标题', async () => {
    render(<LogsTab />);
    await waitFor(() => {
      expect(screen.getByText('日志')).toBeInTheDocument();
    });
  });

  it('应该显示日志条目', async () => {
    render(<LogsTab />);
    await waitFor(() => {
      expect(screen.getByText('用户登录成功')).toBeInTheDocument();
      expect(screen.getByText('DLP 规则匹配')).toBeInTheDocument();
      expect(screen.getByText('连接超时')).toBeInTheDocument();
    });
  });

  it('应该显示级别标签', async () => {
    render(<LogsTab />);
    await waitFor(() => {
      expect(screen.getAllByText('信息').length).toBeGreaterThan(0);
      expect(screen.getAllByText('警告').length).toBeGreaterThan(0);
      expect(screen.getAllByText('错误').length).toBeGreaterThan(0);
    });
  });

  it('应该显示模块名称', async () => {
    render(<LogsTab />);
    await waitFor(() => {
      expect(screen.getAllByText('auth').length).toBeGreaterThan(0);
      expect(screen.getAllByText('dlp').length).toBeGreaterThan(0);
      expect(screen.getAllByText('network').length).toBeGreaterThan(0);
    });
  });

  it('应该显示搜索框', async () => {
    render(<LogsTab />);
    await waitFor(() => {
      expect(screen.getByPlaceholderText('搜索日志...')).toBeInTheDocument();
    });
  });

  it('应该显示导出和清空按钮', async () => {
    render(<LogsTab />);
    await waitFor(() => {
      expect(screen.getByText('导出')).toBeInTheDocument();
      expect(screen.getByText('清空')).toBeInTheDocument();
    });
  });

  it('应该显示实时流按钮', async () => {
    render(<LogsTab />);
    await waitFor(() => {
      expect(screen.getByText('实时流：关')).toBeInTheDocument();
    });
  });

  it('点击实时流按钮应切换状态', async () => {
    render(<LogsTab />);
    await waitFor(() => {
      expect(screen.getByText('实时流：关')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByText('实时流：关'));
    expect(screen.getByText('实时流：开')).toBeInTheDocument();
  });

  it('应该显示级别过滤下拉框', async () => {
    render(<LogsTab />);
    await waitFor(() => {
      const select = screen.getByDisplayValue('所有级别');
      expect(select).toBeInTheDocument();
    });
  });

  // ── 单元测试（错误路径）──

  it('test_failure_api_error_shows_error_message', async () => {
    const { logApi } = await import('../../../utils/tauri');
    vi.mocked(logApi.getLogs).mockRejectedValueOnce(new Error('Network error'));
    render(<LogsTab />);
    await waitFor(() => {
      expect(screen.getByText('加载日志失败')).toBeInTheDocument();
    });
  });

  it('test_failure_empty_logs_shows_empty_state', async () => {
    const { logApi } = await import('../../../utils/tauri');
    vi.mocked(logApi.getLogs).mockResolvedValueOnce([]);
    render(<LogsTab />);
    await waitFor(() => {
      expect(screen.getByText('没有找到匹配的日志')).toBeInTheDocument();
    });
  });

  // ── 契约测试 ──

  it('test_contract_log_entry_displays_all_fields', async () => {
    render(<LogsTab />);
    await waitFor(() => {
      // 每条日志应显示：级别标签、模块、时间、消息
      expect(screen.getAllByText('信息').length).toBeGreaterThan(0);
      expect(screen.getAllByText('auth').length).toBeGreaterThan(0);
      expect(screen.getByText('用户登录成功')).toBeInTheDocument();
    });
  });

  it('test_contract_filter_options_match_design', async () => {
    render(<LogsTab />);
    await waitFor(() => {
      const levelSelect = screen.getByDisplayValue('所有级别');
      expect(levelSelect).toBeInTheDocument();
      // 验证选项存在
      const options = levelSelect.querySelectorAll('option');
      const values = Array.from(options).map((o) => o.value);
      expect(values).toContain('all');
      expect(values).toContain('info');
      expect(values).toContain('warn');
      expect(values).toContain('error');
      expect(values).toContain('debug');
    });
  });

  // ── 安全审计测试 ──

  it('test_audit_no_sensitive_data_in_rendered_output', async () => {
    render(<LogsTab />);
    await waitFor(() => {
      const html = document.body.innerHTML;
      expect(html).not.toContain('password');
      expect(html).not.toContain('secret');
      expect(html).not.toContain('api_key');
    });
  });

  it('test_audit_clear_requires_confirmation', async () => {
    const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(false);
    render(<LogsTab />);
    await waitFor(() => {
      expect(screen.getByText('清空')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByText('清空'));
    expect(confirmSpy).toHaveBeenCalled();
    confirmSpy.mockRestore();
  });
});
