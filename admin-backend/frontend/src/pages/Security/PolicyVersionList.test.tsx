import React from 'react';
import { render, screen, waitFor, fireEvent, act } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { message } from 'antd';
import { PolicyVersionList } from './PolicyVersionList';

// Mock antd 的 DatePicker（避免 dayjs 问题）
vi.mock('antd', async () => {
  const actual = await vi.importActual('antd');
  return {
    ...actual,
    DatePicker: {
      ...(actual as any).DatePicker,
      RangePicker: ({ placeholder, onChange, value, ...props }: any) => (
        <div data-testid="range-picker">
          <input
            placeholder={placeholder?.[0] || '开始日期'}
            data-testid="range-picker-start"
            onChange={(e) => onChange?.([e.target.value, value?.[1]])}
          />
          <input
            placeholder={placeholder?.[1] || '结束日期'}
            data-testid="range-picker-end"
            onChange={(e) => onChange?.([value?.[0], e.target.value])}
          />
        </div>
      ),
    },
  };
});

// Mock API client
vi.mock('../../api/client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    put: vi.fn(),
    delete: vi.fn(),
  },
}));

import { apiClient } from '../../api/client';

const mockRecords = [
  {
    id: '1',
    rule_id: 'rule-1',
    rule_type: 'dlp_rule',
    rule_name: '身份证号检测',
    change_type: 'create',
    field_changed: null,
    old_value: null,
    new_value: { name: '身份证号检测', pattern: '\\d{18}', severity: 'high' },
    changed_by: 'user-1',
    changed_by_name: 'admin',
    changed_at: new Date().toISOString(),
    reason: null,
  },
  {
    id: '2',
    rule_id: 'rule-2',
    rule_type: 'sensitive_op',
    rule_name: '文件导出',
    change_type: 'update',
    field_changed: 'risk_level',
    old_value: 'medium',
    new_value: 'high',
    changed_by: 'user-1',
    changed_by_name: 'admin',
    changed_at: new Date(Date.now() - 3600000).toISOString(),
    reason: '提升风险等级',
  },
  {
    id: '3',
    rule_id: 'rule-3',
    rule_type: 'dictionary',
    rule_name: '敏感词字典',
    change_type: 'delete',
    field_changed: null,
    old_value: { name: '敏感词字典', keywords: ['机密', '绝密'] },
    new_value: null,
    changed_by: 'user-2',
    changed_by_name: 'operator',
    changed_at: new Date(Date.now() - 86400000).toISOString(),
    reason: null,
  },
  {
    id: '4',
    rule_id: 'rule-4',
    rule_type: 'dlp_rule',
    rule_name: '手机号检测',
    change_type: 'enable',
    field_changed: 'enabled',
    old_value: false,
    new_value: true,
    changed_by: 'user-1',
    changed_by_name: 'admin',
    changed_at: new Date(Date.now() - 172800000).toISOString(),
    reason: null,
  },
  {
    id: '5',
    rule_id: 'rule-5',
    rule_type: 'dlp_rule',
    rule_name: '邮箱检测',
    change_type: 'disable',
    field_changed: 'enabled',
    old_value: true,
    new_value: false,
    changed_by: 'user-1',
    changed_by_name: 'admin',
    changed_at: new Date(Date.now() - 259200000).toISOString(),
    reason: null,
  },
];

const mockStats = {
  total: 42,
  by_change_type: [
    { type: 'create', count: 15 },
    { type: 'update', count: 12 },
    { type: 'delete', count: 8 },
    { type: 'enable', count: 4 },
    { type: 'disable', count: 3 },
  ],
  by_rule_type: [
    { type: 'dlp_rule', count: 25 },
    { type: 'sensitive_op', count: 10 },
    { type: 'dictionary', count: 7 },
  ],
  trend_7d: [
    { date: '2026-03-15', count: 3 },
    { date: '2026-03-16', count: 5 },
    { date: '2026-03-17', count: 2 },
  ],
};

function setupMocks(overrides?: { records?: any[]; stats?: any; error?: boolean }) {
  const records = overrides?.records ?? mockRecords;
  const stats = overrides?.stats ?? mockStats;

  (apiClient.get as any).mockImplementation((url: string) => {
    if (overrides?.error) return Promise.reject({ response: { data: { error: '服务器错误' } } });
    if (url.includes('/policy-changes/stats')) return Promise.resolve({ data: stats });
    if (url.includes('/policy-changes')) {
      return Promise.resolve({
        data: { records, total: records.length, page: 1, page_size: 20, total_pages: 1 },
      });
    }
    return Promise.resolve({ data: {} });
  });
}

describe('PolicyVersionList', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  // ============================================================================
  // 单元测试 - 正常路径
  // ============================================================================

  describe('单元测试 - 正常路径', () => {
    it('应该正确渲染页面标题', async () => {
      setupMocks();
      render(<PolicyVersionList />);
      expect(screen.getByText('策略变更记录')).toBeInTheDocument();
    });

    it('应该加载并显示变更记录', async () => {
      setupMocks();
      render(<PolicyVersionList />);
      await waitFor(() => {
        expect(screen.getByText('身份证号检测')).toBeInTheDocument();
        expect(screen.getByText('文件导出')).toBeInTheDocument();
        expect(screen.getByText('敏感词字典')).toBeInTheDocument();
      });
    });

    it('应该显示统计卡片', async () => {
      setupMocks();
      render(<PolicyVersionList />);
      await waitFor(() => {
        expect(screen.getByText('总变更次数')).toBeInTheDocument();
        expect(screen.getByText('42')).toBeInTheDocument();
      });
    });

    it('应该显示变更类型标签', async () => {
      setupMocks();
      render(<PolicyVersionList />);
      await waitFor(() => {
        expect(screen.getByText('创建')).toBeInTheDocument();
        expect(screen.getByText('修改')).toBeInTheDocument();
        expect(screen.getByText('删除')).toBeInTheDocument();
      });
    });

    it('应该显示规则类型标签', async () => {
      setupMocks();
      render(<PolicyVersionList />);
      await waitFor(() => {
        expect(screen.getAllByText('DLP 规则').length).toBeGreaterThan(0);
        expect(screen.getByText('敏感操作')).toBeInTheDocument();
        expect(screen.getByText('字典')).toBeInTheDocument();
      });
    });

    it('应该显示操作人信息', async () => {
      setupMocks();
      render(<PolicyVersionList />);
      await waitFor(() => {
        expect(screen.getAllByText('admin').length).toBeGreaterThan(0);
        expect(screen.getByText('operator')).toBeInTheDocument();
      });
    });

    it('应该显示记录总数', async () => {
      setupMocks();
      render(<PolicyVersionList />);
      await waitFor(() => {
        expect(screen.getByText(/共 5 条记录/)).toBeInTheDocument();
      });
    });

    it('应该显示 update 类型的 diff 信息', async () => {
      setupMocks();
      render(<PolicyVersionList />);
      await waitFor(() => {
        expect(screen.getByText('变更字段：风险等级')).toBeInTheDocument();
        expect(screen.getByText('medium')).toBeInTheDocument();
        expect(screen.getByText('high')).toBeInTheDocument();
      });
    });

    it('应该显示变更原因', async () => {
      setupMocks();
      render(<PolicyVersionList />);
      await waitFor(() => {
        expect(screen.getByText('提升风险等级')).toBeInTheDocument();
      });
    });
  });

  // ============================================================================
  // 单元测试 - 筛选功能
  // ============================================================================

  describe('单元测试 - 筛选功能', () => {
    it('应该渲染搜索框', async () => {
      setupMocks();
      render(<PolicyVersionList />);
      expect(screen.getByPlaceholderText('搜索规则名称或操作人')).toBeInTheDocument();
    });

    it('应该渲染规则类型筛选器', async () => {
      setupMocks();
      render(<PolicyVersionList />);
      expect(screen.getByText('规则类型')).toBeInTheDocument();
    });

    it('应该渲染变更类型筛选器', async () => {
      setupMocks();
      render(<PolicyVersionList />);
      expect(screen.getByText('变更类型')).toBeInTheDocument();
    });

    it('搜索时应该触发 API 调用', async () => {
      setupMocks();
      render(<PolicyVersionList />);
      await waitFor(() => expect(apiClient.get).toHaveBeenCalled());

      const searchInput = screen.getByPlaceholderText('搜索规则名称或操作人');
      fireEvent.change(searchInput, { target: { value: '身份证' } });

      await waitFor(() => {
        const calls = (apiClient.get as any).mock.calls;
        const lastCall = calls[calls.length - 1];
        expect(lastCall[1]?.params?.search).toBe('身份证');
      });
    });

    it('清除筛选按钮应该重置所有筛选条件', async () => {
      setupMocks();
      render(<PolicyVersionList />);
      await waitFor(() => expect(apiClient.get).toHaveBeenCalled());

      // 输入搜索文本
      const searchInput = screen.getByPlaceholderText('搜索规则名称或操作人');
      fireEvent.change(searchInput, { target: { value: 'test' } });

      await waitFor(() => {
        expect(screen.getByText('清除筛选')).toBeInTheDocument();
      });

      fireEvent.click(screen.getByText('清除筛选'));

      await waitFor(() => {
        expect((searchInput as HTMLInputElement).value).toBe('');
      });
    });
  });

  // ============================================================================
  // 单元测试 - 失败路径
  // ============================================================================

  describe('单元测试 - 失败路径', () => {
    it('API 失败时应该显示错误消息', async () => {
      setupMocks({ error: true });
      const spy = vi.spyOn(message, 'error');
      render(<PolicyVersionList />);
      await waitFor(() => {
        expect(spy).toHaveBeenCalledWith('服务器错误');
      });
    });

    it('空数据时应该显示空状态', async () => {
      setupMocks({ records: [] });
      render(<PolicyVersionList />);
      await waitFor(() => {
        expect(screen.getByText(/暂无策略变更记录/)).toBeInTheDocument();
      });
    });

    it('筛选无结果时应该显示清除筛选提示', async () => {
      setupMocks({ records: [] });
      render(<PolicyVersionList />);
      await waitFor(() => expect(apiClient.get).toHaveBeenCalled());

      const searchInput = screen.getByPlaceholderText('搜索规则名称或操作人');
      fireEvent.change(searchInput, { target: { value: '不存在的规则' } });

      // 模拟返回空结果
      (apiClient.get as any).mockImplementation((url: string) => {
        if (url.includes('/policy-changes/stats')) return Promise.resolve({ data: mockStats });
        return Promise.resolve({ data: { records: [], total: 0, page: 1, page_size: 20, total_pages: 0 } });
      });

      await waitFor(() => {
        expect(screen.getByText(/没有匹配的变更记录/)).toBeInTheDocument();
      });
    });

    it('网络错误时应该优雅处理', async () => {
      (apiClient.get as any).mockRejectedValue(new Error('Network Error'));
      const spy = vi.spyOn(message, 'error');
      render(<PolicyVersionList />);
      await waitFor(() => {
        expect(spy).toHaveBeenCalled();
      });
    });
  });

  // ============================================================================
  // 集成测试
  // ============================================================================

  describe('集成测试', () => {
    it('刷新按钮应该重新加载数据和统计', async () => {
      setupMocks();
      render(<PolicyVersionList />);
      await waitFor(() => expect(apiClient.get).toHaveBeenCalled());

      const callCount = (apiClient.get as any).mock.calls.length;
      fireEvent.click(screen.getByText('刷新'));

      await waitFor(() => {
        expect((apiClient.get as any).mock.calls.length).toBeGreaterThan(callCount);
      });
    });

    it('筛选条件变化应该重置页码并重新加载', async () => {
      setupMocks();
      render(<PolicyVersionList />);
      await waitFor(() => expect(apiClient.get).toHaveBeenCalled());

      const searchInput = screen.getByPlaceholderText('搜索规则名称或操作人');
      fireEvent.change(searchInput, { target: { value: 'DLP' } });

      await waitFor(() => {
        const calls = (apiClient.get as any).mock.calls;
        const lastPolicyCall = calls.filter((c: any) => c[0] === '/policy-changes').pop();
        expect(lastPolicyCall?.[1]?.params?.page).toBe(1);
      });
    });

    it('统计数据和列表数据应该同时加载', async () => {
      setupMocks();
      render(<PolicyVersionList />);
      await waitFor(() => {
        const calls = (apiClient.get as any).mock.calls.map((c: any) => c[0]);
        expect(calls).toContain('/policy-changes');
        expect(calls).toContain('/policy-changes/stats');
      });
    });
  });

  // ============================================================================
  // 安全测试
  // ============================================================================

  describe('安全测试', () => {
    it('应该正确转义 HTML 内容防止 XSS', async () => {
      const xssRecords = [{
        ...mockRecords[0],
        rule_name: '<script>alert("xss")</script>',
        changed_by_name: '<img onerror="alert(1)" src="x">',
      }];
      setupMocks({ records: xssRecords });
      render(<PolicyVersionList />);
      await waitFor(() => {
        // React 自动转义，不应该执行脚本
        const container = document.querySelector('.policy-timeline-container');
        expect(container?.innerHTML).not.toContain('<script>');
      });
    });

    it('应该安全处理 null/undefined 值', async () => {
      const nullRecords = [{
        id: '1',
        rule_id: 'rule-1',
        rule_type: 'dlp_rule',
        rule_name: null,
        change_type: 'update',
        field_changed: null,
        old_value: null,
        new_value: null,
        changed_by: null,
        changed_by_name: null,
        changed_at: new Date().toISOString(),
        reason: null,
      }];
      setupMocks({ records: nullRecords });
      render(<PolicyVersionList />);
      await waitFor(() => {
        // 不应该崩溃
        expect(screen.getByText('策略变更记录')).toBeInTheDocument();
      });
    });
  });

  // ============================================================================
  // 需求级测试
  // ============================================================================

  describe('需求级测试', () => {
    it('req_policy_001: 应该以时间线形式展示变更记录', async () => {
      setupMocks();
      render(<PolicyVersionList />);
      await waitFor(() => {
        // 验证时间线组件存在
        const timeline = document.querySelector('.ant-timeline');
        expect(timeline).toBeInTheDocument();
      });
    });

    it('req_policy_002: 应该支持按规则类型筛选', async () => {
      setupMocks();
      render(<PolicyVersionList />);
      expect(screen.getByText('规则类型')).toBeInTheDocument();
    });

    it('req_policy_003: 应该支持按变更类型筛选', async () => {
      setupMocks();
      render(<PolicyVersionList />);
      expect(screen.getByText('变更类型')).toBeInTheDocument();
    });

    it('req_policy_004: 应该支持日期范围筛选', async () => {
      setupMocks();
      render(<PolicyVersionList />);
      expect(screen.getByTestId('range-picker')).toBeInTheDocument();
    });

    it('req_policy_005: 应该展示变更前后的 diff 对比', async () => {
      setupMocks();
      render(<PolicyVersionList />);
      await waitFor(() => {
        // update 类型应该显示 diff
        expect(screen.getByText('变更字段：风险等级')).toBeInTheDocument();
        // 旧值和新值
        const diffOld = document.querySelector('.diff-old');
        const diffNew = document.querySelector('.diff-new');
        expect(diffOld).toBeInTheDocument();
        expect(diffNew).toBeInTheDocument();
      });
    });

    it('req_policy_006: 应该显示操作人信息', async () => {
      setupMocks();
      render(<PolicyVersionList />);
      await waitFor(() => {
        expect(screen.getAllByText('admin').length).toBeGreaterThan(0);
      });
    });

    it('req_policy_007: 应该显示统计概览', async () => {
      setupMocks();
      render(<PolicyVersionList />);
      await waitFor(() => {
        expect(screen.getByText('总变更次数')).toBeInTheDocument();
      });
    });

    it('req_policy_008: create 类型应该显示创建内容', async () => {
      setupMocks();
      render(<PolicyVersionList />);
      await waitFor(() => {
        expect(screen.getByText('创建内容')).toBeInTheDocument();
      });
    });

    it('req_policy_009: delete 类型应该显示删除内容', async () => {
      setupMocks();
      render(<PolicyVersionList />);
      await waitFor(() => {
        expect(screen.getByText('删除内容')).toBeInTheDocument();
      });
    });

    it('req_policy_010: enable/disable 类型应该显示状态标签', async () => {
      setupMocks();
      render(<PolicyVersionList />);
      await waitFor(() => {
        expect(screen.getByText('已启用')).toBeInTheDocument();
        expect(screen.getByText('已禁用')).toBeInTheDocument();
      });
    });
  });

  // ============================================================================
  // 契约测试
  // ============================================================================

  describe('契约测试', () => {
    it('应该使用正确的 API 端点获取变更记录', async () => {
      setupMocks();
      render(<PolicyVersionList />);
      await waitFor(() => {
        expect(apiClient.get).toHaveBeenCalledWith('/policy-changes', expect.any(Object));
      });
    });

    it('应该使用正确的 API 端点获取统计数据', async () => {
      setupMocks();
      render(<PolicyVersionList />);
      await waitFor(() => {
        expect(apiClient.get).toHaveBeenCalledWith('/policy-changes/stats');
      });
    });

    it('应该传递正确的分页参数', async () => {
      setupMocks();
      render(<PolicyVersionList />);
      await waitFor(() => {
        expect(apiClient.get).toHaveBeenCalledWith('/policy-changes', {
          params: expect.objectContaining({ page: 1, page_size: 20 }),
        });
      });
    });

    it('应该传递筛选参数到 API', async () => {
      setupMocks();
      render(<PolicyVersionList />);
      await waitFor(() => expect(apiClient.get).toHaveBeenCalled());

      const searchInput = screen.getByPlaceholderText('搜索规则名称或操作人');
      fireEvent.change(searchInput, { target: { value: 'test' } });

      await waitFor(() => {
        const calls = (apiClient.get as any).mock.calls;
        const lastPolicyCall = calls.filter((c: any) => c[0] === '/policy-changes').pop();
        expect(lastPolicyCall?.[1]?.params?.search).toBe('test');
      });
    });

    it('应该正确处理 API 返回的数据结构', async () => {
      const apiResponse = {
        records: mockRecords,
        total: mockRecords.length,
        page: 1,
        page_size: 20,
        total_pages: 1,
      };
      (apiClient.get as any).mockImplementation((url: string) => {
        if (url.includes('/stats')) return Promise.resolve({ data: mockStats });
        return Promise.resolve({ data: apiResponse });
      });

      render(<PolicyVersionList />);
      await waitFor(() => {
        expect(screen.getByText('身份证号检测')).toBeInTheDocument();
        expect(screen.getByText(/共 5 条记录/)).toBeInTheDocument();
      });
    });
  });

  // ============================================================================
  // 边界情况测试
  // ============================================================================

  describe('边界情况测试', () => {
    it('应该处理空统计数据', async () => {
      setupMocks({ stats: { total: 0, by_change_type: [], by_rule_type: [], trend_7d: [] } });
      render(<PolicyVersionList />);
      await waitFor(() => {
        expect(screen.getByText('策略变更记录')).toBeInTheDocument();
      });
    });

    it('应该处理超长规则名称', async () => {
      const longNameRecords = [{
        ...mockRecords[0],
        rule_name: 'A'.repeat(200),
      }];
      setupMocks({ records: longNameRecords });
      render(<PolicyVersionList />);
      await waitFor(() => {
        expect(screen.getByText('A'.repeat(200))).toBeInTheDocument();
      });
    });

    it('应该处理特殊字符的规则名称', async () => {
      const specialRecords = [{
        ...mockRecords[0],
        rule_name: '规则 <>&"\'',
      }];
      setupMocks({ records: specialRecords });
      render(<PolicyVersionList />);
      await waitFor(() => {
        expect(screen.getByText('规则 <>&"\'')).toBeInTheDocument();
      });
    });

    it('应该处理复杂的 JSON diff 值', async () => {
      const complexRecords = [{
        ...mockRecords[1],
        old_value: { keywords: ['a', 'b', 'c'], match_mode: 'exact' },
        new_value: { keywords: ['a', 'b', 'c', 'd'], match_mode: 'contains' },
      }];
      setupMocks({ records: complexRecords });
      render(<PolicyVersionList />);
      await waitFor(() => {
        expect(screen.getByText('文件导出')).toBeInTheDocument();
      });
    });

    it('统计加载失败不应该影响列表显示', async () => {
      (apiClient.get as any).mockImplementation((url: string) => {
        if (url.includes('/stats')) return Promise.reject(new Error('stats error'));
        return Promise.resolve({
          data: { records: mockRecords, total: mockRecords.length, page: 1, page_size: 20, total_pages: 1 },
        });
      });

      render(<PolicyVersionList />);
      await waitFor(() => {
        expect(screen.getByText('身份证号检测')).toBeInTheDocument();
      });
    });
  });
});
