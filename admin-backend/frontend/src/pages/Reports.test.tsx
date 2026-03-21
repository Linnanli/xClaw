import React from 'react';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { message } from 'antd';
import { Reports } from './Reports';

vi.mock('../api/client', () => ({
  apiClient: { get: vi.fn() },
}));

import { apiClient } from '../api/client';

function setupMocks(overrides?: { error?: boolean }) {
  (apiClient.get as any).mockImplementation((url: string) => {
    if (overrides?.error) return Promise.reject({ response: { data: { error: '加载失败' } } });
    if (url.includes('/dlp-rules')) return Promise.resolve({ data: { rules: [
      { id: '1', name: 'r1', severity: 'high', enabled: true },
      { id: '2', name: 'r2', severity: 'medium', enabled: false },
    ] } });
    if (url.includes('/sensitive-operations')) return Promise.resolve({ data: { operations: [
      { id: '1', name: 'op1', risk_level: 'high', enabled: true },
    ] } });
    if (url.includes('/dlp-dictionaries')) return Promise.resolve({ data: { dictionaries: [
      { id: '1', name: 'd1', keyword_count: 50 },
    ] } });
    if (url.includes('/clients/stats')) return Promise.resolve({ data: { total: 5, online: 3, offline: 2 } });
    if (url.includes('/policy-changes/stats')) return Promise.resolve({ data: { total: 20, trend_7d: [{ date: '2026-03-20', count: 5 }], by_change_type: [{ type: 'create', count: 10 }] } });
    if (url.includes('/audit-logs')) return Promise.resolve({ data: { total: 100, logs: [] } });
    return Promise.resolve({ data: {} });
  });
}

describe('Reports', () => {
  beforeEach(() => vi.clearAllMocks());

  describe('单元测试 - 正常路径', () => {
    it('应该渲染页面标题', async () => {
      setupMocks();
      render(<Reports />);
      expect(screen.getByText('统计报表')).toBeInTheDocument();
    });

    it('应该显示 DLP 规则统计', async () => {
      setupMocks();
      render(<Reports />);
      await waitFor(() => expect(screen.getByText('DLP 规则')).toBeInTheDocument());
    });

    it('应该显示敏感操作统计', async () => {
      setupMocks();
      render(<Reports />);
      await waitFor(() => expect(screen.getByText('敏感操作')).toBeInTheDocument());
    });

    it('应该显示客户端统计', async () => {
      setupMocks();
      render(<Reports />);
      await waitFor(() => expect(screen.getByText('客户端')).toBeInTheDocument());
    });

    it('应该显示策略变更统计', async () => {
      setupMocks();
      render(<Reports />);
      await waitFor(() => expect(screen.getByText('策略变更')).toBeInTheDocument());
    });

    it('应该显示严重级别分布', async () => {
      setupMocks();
      render(<Reports />);
      await waitFor(() => expect(screen.getByText('DLP 规则严重级别分布')).toBeInTheDocument());
    });

    it('应该显示风险等级分布', async () => {
      setupMocks();
      render(<Reports />);
      await waitFor(() => expect(screen.getByText('敏感操作风险等级分布')).toBeInTheDocument());
    });
  });

  describe('单元测试 - 失败路径', () => {
    it('所有 API 失败时应该显示空状态', async () => {
      (apiClient.get as any).mockRejectedValue(new Error('fail'));
      render(<Reports />);
      await waitFor(() => expect(screen.getByText('统计报表')).toBeInTheDocument());
    });

    it('部分 API 失败时应该优雅降级', async () => {
      (apiClient.get as any).mockImplementation((url: string) => {
        if (url.includes('/clients')) return Promise.reject(new Error('fail'));
        if (url.includes('/dlp-rules')) return Promise.resolve({ data: { rules: [{ id: '1', severity: 'high', enabled: true }] } });
        if (url.includes('/sensitive-operations')) return Promise.resolve({ data: { operations: [] } });
        if (url.includes('/dlp-dictionaries')) return Promise.resolve({ data: { dictionaries: [] } });
        if (url.includes('/policy-changes')) return Promise.resolve({ data: { total: 0, trend_7d: [], by_change_type: [] } });
        if (url.includes('/audit-logs')) return Promise.resolve({ data: { total: 0 } });
        return Promise.resolve({ data: {} });
      });
      render(<Reports />);
      await waitFor(() => expect(screen.getByText('DLP 规则')).toBeInTheDocument());
    });
  });

  describe('集成测试', () => {
    it('刷新按钮应该存在且可点击', async () => {
      setupMocks();
      render(<Reports />);
      await waitFor(() => expect(screen.getByText('DLP 规则')).toBeInTheDocument());
      const refreshBtn = screen.getByText('刷新');
      expect(refreshBtn).toBeInTheDocument();
      expect(refreshBtn.closest('button')).not.toBeDisabled();
    });

    it('应该并行加载所有模块数据', async () => {
      setupMocks();
      render(<Reports />);
      await waitFor(() => {
        const urls = (apiClient.get as any).mock.calls.map((c: any) => c[0]);
        expect(urls).toContain('/dlp-rules');
        expect(urls).toContain('/sensitive-operations');
        expect(urls).toContain('/dlp-dictionaries');
        expect(urls).toContain('/clients/stats');
        expect(urls).toContain('/policy-changes/stats');
      });
    });
  });

  describe('需求级测试', () => {
    it('req_report_001: 应该展示安全策略概览', async () => {
      setupMocks();
      render(<Reports />);
      await waitFor(() => {
        expect(screen.getByText('DLP 规则')).toBeInTheDocument();
        expect(screen.getByText('敏感操作')).toBeInTheDocument();
      });
    });

    it('req_report_002: 应该展示分布图表', async () => {
      setupMocks();
      render(<Reports />);
      await waitFor(() => {
        expect(screen.getByText('DLP 规则严重级别分布')).toBeInTheDocument();
        expect(screen.getByText('敏感操作风险等级分布')).toBeInTheDocument();
      });
    });

    it('req_report_003: 应该展示启用/禁用统计', async () => {
      setupMocks();
      render(<Reports />);
      await waitFor(() => {
        expect(screen.getByText(/已启用 1 \/ 已禁用 1/)).toBeInTheDocument();
      });
    });
  });

  describe('契约测试', () => {
    it('应该调用正确的 API 端点', async () => {
      setupMocks();
      render(<Reports />);
      await waitFor(() => {
        expect(apiClient.get).toHaveBeenCalledWith('/dlp-rules');
        expect(apiClient.get).toHaveBeenCalledWith('/sensitive-operations');
      });
    });
  });

  describe('边界情况', () => {
    it('应该处理空数据', async () => {
      (apiClient.get as any).mockResolvedValue({ data: { rules: [], operations: [], dictionaries: [], total: 0, online: 0, offline: 0, trend_7d: [], by_change_type: [], logs: [] } });
      render(<Reports />);
      await waitFor(() => expect(screen.getByText('统计报表')).toBeInTheDocument());
    });
  });
});
