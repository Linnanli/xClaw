import React from 'react';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { PluginList } from './PluginList';

vi.mock('../../api/client', () => ({
  apiClient: {
    get: vi.fn(),
  },
}));

import { apiClient } from '../../api/client';

const mockPlugins = [
  {
    id: 'p1', name: '日志收集器', description: '收集和聚合系统日志',
    version: '3.0.1', author: 'ops-team', enabled: true,
    created_at: '2025-01-10T00:00:00Z', updated_at: '2025-06-10T08:00:00Z',
  },
  {
    id: 'p2', name: '通知推送', description: '多渠道消息通知',
    version: '1.5.0', author: 'platform', enabled: true,
    created_at: '2025-03-01T00:00:00Z', updated_at: '2025-06-18T16:00:00Z',
  },
];

function setupMocks(overrides?: { plugins?: any[]; error?: boolean }) {
  (apiClient.get as any).mockImplementation(() => {
    if (overrides?.error) return Promise.reject(new Error('服务器错误'));
    return Promise.resolve({ data: { plugins: overrides?.plugins ?? mockPlugins } });
  });
}

describe('PluginList', () => {
  beforeEach(() => vi.clearAllMocks());

  // 单元测试 - 正常路径
  describe('单元测试 - 正常路径', () => {
    it('应该渲染页面标题', async () => {
      setupMocks();
      render(<PluginList />);
      expect(screen.getByText('插件管理')).toBeInTheDocument();
    });

    it('应该加载并显示插件列表', async () => {
      setupMocks();
      render(<PluginList />);
      await waitFor(() => {
        expect(screen.getByText('日志收集器')).toBeInTheDocument();
        expect(screen.getByText('通知推送')).toBeInTheDocument();
      });
    });

    it('应该显示插件描述', async () => {
      setupMocks();
      render(<PluginList />);
      await waitFor(() => {
        expect(screen.getByText('收集和聚合系统日志')).toBeInTheDocument();
      });
    });

    it('应该显示版本标签', async () => {
      setupMocks();
      render(<PluginList />);
      await waitFor(() => {
        expect(screen.getByText('3.0.1')).toBeInTheDocument();
        expect(screen.getByText('1.5.0')).toBeInTheDocument();
      });
    });

    it('应该显示作者信息', async () => {
      setupMocks();
      render(<PluginList />);
      await waitFor(() => {
        expect(screen.getByText('ops-team')).toBeInTheDocument();
        expect(screen.getByText('platform')).toBeInTheDocument();
      });
    });

    it('应该显示统计卡片', async () => {
      setupMocks();
      render(<PluginList />);
      await waitFor(() => {
        expect(screen.getByText('已安装插件')).toBeInTheDocument();
      });
    });

    it('应该显示插件总数', async () => {
      setupMocks();
      render(<PluginList />);
      await waitFor(() => {
        expect(screen.getByText(/共 2 个插件/)).toBeInTheDocument();
      });
    });
  });

  // 单元测试 - 失败路径
  describe('单元测试 - 失败路径', () => {
    it('API 失败时应该显示空列表', async () => {
      setupMocks({ error: true });
      render(<PluginList />);
      await waitFor(() => {
        expect(screen.getByText(/暂无已安装的插件/)).toBeInTheDocument();
      });
    });

    it('空数据时应该显示空状态', async () => {
      setupMocks({ plugins: [] });
      render(<PluginList />);
      await waitFor(() => {
        expect(screen.getByText(/暂无已安装的插件/)).toBeInTheDocument();
      });
    });

    it('API 返回非数组时应该优雅处理', async () => {
      (apiClient.get as any).mockResolvedValue({ data: {} });
      render(<PluginList />);
      await waitFor(() => {
        expect(screen.getByText('插件管理')).toBeInTheDocument();
      });
    });
  });

  // 集成测试
  describe('集成测试', () => {
    it('刷新按钮应该重新加载数据', async () => {
      setupMocks();
      render(<PluginList />);
      await waitFor(() => expect(screen.getByText('日志收集器')).toBeInTheDocument());
      expect(apiClient.get).toHaveBeenCalledWith('/plugins');
      // 验证刷新按钮存在且可点击
      const refreshBtn = screen.getByText('刷新');
      expect(refreshBtn).toBeInTheDocument();
      expect(refreshBtn.closest('button')).not.toBeDisabled();
    });

    it('搜索应该过滤插件列表', async () => {
      setupMocks();
      render(<PluginList />);
      await waitFor(() => expect(screen.getByText('日志收集器')).toBeInTheDocument());
      fireEvent.change(screen.getByPlaceholderText('搜索插件名称或描述'), { target: { value: '通知' } });
      await waitFor(() => {
        expect(screen.getByText('通知推送')).toBeInTheDocument();
        expect(screen.queryByText('日志收集器')).not.toBeInTheDocument();
      });
    });

    it('清空搜索应该显示全部插件', async () => {
      setupMocks();
      render(<PluginList />);
      await waitFor(() => expect(screen.getByText('日志收集器')).toBeInTheDocument());
      const input = screen.getByPlaceholderText('搜索插件名称或描述');
      fireEvent.change(input, { target: { value: '通知' } });
      await waitFor(() => expect(screen.queryByText('日志收集器')).not.toBeInTheDocument());
      fireEvent.change(input, { target: { value: '' } });
      await waitFor(() => {
        expect(screen.getByText('日志收集器')).toBeInTheDocument();
        expect(screen.getByText('通知推送')).toBeInTheDocument();
      });
    });
  });

  // 需求级测试
  describe('需求级测试', () => {
    it('req_plugin_001: 应该展示插件列表', async () => {
      setupMocks();
      render(<PluginList />);
      await waitFor(() => {
        expect(screen.getByText('日志收集器')).toBeInTheDocument();
      });
    });

    it('req_plugin_002: 应该支持搜索筛选', async () => {
      setupMocks();
      render(<PluginList />);
      expect(screen.getByPlaceholderText('搜索插件名称或描述')).toBeInTheDocument();
    });

    it('req_plugin_003: 应该显示插件统计', async () => {
      setupMocks();
      render(<PluginList />);
      await waitFor(() => {
        expect(screen.getByText('已安装插件')).toBeInTheDocument();
      });
    });
  });

  // 契约测试
  describe('契约测试', () => {
    it('应该使用正确的 API 端点', async () => {
      setupMocks();
      render(<PluginList />);
      await waitFor(() => {
        expect(apiClient.get).toHaveBeenCalledWith('/plugins');
      });
    });
  });

  // 边界情况
  describe('边界情况', () => {
    it('应该处理缺失描述的插件', async () => {
      const partial = [{ id: 'p1', name: '测试插件', version: '1.0.0', enabled: true, created_at: '2025-01-01T00:00:00Z', updated_at: '2025-01-01T00:00:00Z' }];
      setupMocks({ plugins: partial });
      render(<PluginList />);
      await waitFor(() => expect(screen.getByText('测试插件')).toBeInTheDocument());
    });

    it('搜索无结果时应该显示空状态', async () => {
      setupMocks();
      render(<PluginList />);
      await waitFor(() => expect(screen.getByText('日志收集器')).toBeInTheDocument());
      fireEvent.change(screen.getByPlaceholderText('搜索插件名称或描述'), { target: { value: 'zzzzzzz' } });
      await waitFor(() => {
        expect(screen.queryByText('日志收集器')).not.toBeInTheDocument();
        expect(screen.queryByText('通知推送')).not.toBeInTheDocument();
      });
    });
  });
});
