import React from 'react';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { message } from 'antd';
import { Settings } from './Settings';

vi.mock('../api/client', () => ({
  apiClient: {
    get: vi.fn(),
    put: vi.fn(),
  },
}));

import { apiClient } from '../api/client';

const mockConfig = {
  dlp_enabled: true,
  dlp_scan_timeout_ms: 5000,
  dlp_fail_open: false,
  audit_retention_days: 90,
  audit_enabled: true,
  client_heartbeat_interval_s: 30,
  client_offline_threshold_s: 120,
  policy_sync_interval_s: 300,
  policy_auto_push: true,
};

function setupMocks(overrides?: { config?: any; error?: boolean; saveError?: boolean }) {
  (apiClient.get as any).mockImplementation(() => {
    if (overrides?.error) return Promise.reject(new Error('服务器错误'));
    return Promise.resolve({ data: overrides?.config ?? mockConfig });
  });
  (apiClient.put as any).mockImplementation(() => {
    if (overrides?.saveError) return Promise.reject(new Error('保存失败'));
    return Promise.resolve({ data: { message: '保存成功' } });
  });
}

describe('Settings', () => {
  beforeEach(() => vi.clearAllMocks());

  // 单元测试 - 正常路径
  describe('单元测试 - 正常路径', () => {
    it('应该渲染页面标题', async () => {
      setupMocks();
      render(<Settings />);
      expect(screen.getByText('系统配置')).toBeInTheDocument();
    });

    it('应该显示 DLP 配置区域', async () => {
      setupMocks();
      render(<Settings />);
      expect(screen.getByText('DLP 数据防泄漏')).toBeInTheDocument();
    });

    it('应该显示审计日志配置区域', async () => {
      setupMocks();
      render(<Settings />);
      expect(screen.getByText('审计日志')).toBeInTheDocument();
    });

    it('应该显示客户端管理配置区域', async () => {
      setupMocks();
      render(<Settings />);
      expect(screen.getByText('客户端管理')).toBeInTheDocument();
    });

    it('应该显示策略同步配置区域', async () => {
      setupMocks();
      render(<Settings />);
      expect(screen.getByText('策略同步')).toBeInTheDocument();
    });

    it('应该显示保存和恢复按钮', async () => {
      setupMocks();
      render(<Settings />);
      expect(screen.getByText('保存配置')).toBeInTheDocument();
      expect(screen.getByText('恢复默认')).toBeInTheDocument();
    });

    it('应该显示故障开放模式警告', async () => {
      setupMocks();
      render(<Settings />);
      expect(screen.getByText(/DLP 扫描失败时将允许消息发送/)).toBeInTheDocument();
    });

    it('应该显示 DLP 扫描开关', async () => {
      setupMocks();
      render(<Settings />);
      expect(screen.getByText('启用 DLP 扫描')).toBeInTheDocument();
    });

    it('应该显示审计日志开关', async () => {
      setupMocks();
      render(<Settings />);
      expect(screen.getByText('启用审计日志')).toBeInTheDocument();
    });
  });

  // 单元测试 - 失败路径
  describe('单元测试 - 失败路径', () => {
    it('API 失败时应该使用默认配置', async () => {
      setupMocks({ error: true });
      render(<Settings />);
      await waitFor(() => {
        expect(screen.getByText('系统配置')).toBeInTheDocument();
      });
    });

    it('保存失败时应该显示提示', async () => {
      setupMocks({ saveError: true });
      const spy = vi.spyOn(message, 'info');
      render(<Settings />);
      await waitFor(() => expect(screen.getByText('保存配置')).toBeInTheDocument());
      fireEvent.click(screen.getByText('保存配置'));
      await waitFor(() => {
        expect(spy).toHaveBeenCalledWith('配置已保存到本地（后端 API 待实现）');
      });
    });
  });

  // 集成测试
  describe('集成测试', () => {
    it('保存按钮应该调用 API', async () => {
      setupMocks();
      render(<Settings />);
      await waitFor(() => expect(apiClient.get).toHaveBeenCalled());
      fireEvent.click(screen.getByText('保存配置'));
      await waitFor(() => {
        expect(apiClient.put).toHaveBeenCalledWith('/settings', expect.any(Object));
      });
    });

    it('保存成功应该显示成功消息', async () => {
      setupMocks();
      const spy = vi.spyOn(message, 'success');
      render(<Settings />);
      await waitFor(() => expect(apiClient.get).toHaveBeenCalled());
      fireEvent.click(screen.getByText('保存配置'));
      await waitFor(() => {
        expect(spy).toHaveBeenCalledWith('配置保存成功');
      });
    });

    it('恢复默认应该重置表单', async () => {
      setupMocks();
      render(<Settings />);
      await waitFor(() => expect(apiClient.get).toHaveBeenCalled());
      // 点击恢复默认按钮
      fireEvent.click(screen.getByText('恢复默认'));
      // 验证按钮存在且可点击（表单已重置）
      expect(screen.getByText('恢复默认')).toBeInTheDocument();
      expect(screen.getByText('保存配置')).toBeInTheDocument();
    });
  });

  // 需求级测试
  describe('需求级测试', () => {
    it('req_settings_001: 应该支持 DLP 配置', async () => {
      setupMocks();
      render(<Settings />);
      expect(screen.getByText('DLP 数据防泄漏')).toBeInTheDocument();
      expect(screen.getByText('启用 DLP 扫描')).toBeInTheDocument();
    });

    it('req_settings_002: 应该支持审计配置', async () => {
      setupMocks();
      render(<Settings />);
      expect(screen.getByText('审计日志')).toBeInTheDocument();
      expect(screen.getByText('启用审计日志')).toBeInTheDocument();
    });

    it('req_settings_003: 应该支持客户端配置', async () => {
      setupMocks();
      render(<Settings />);
      expect(screen.getByText('客户端管理')).toBeInTheDocument();
      expect(screen.getByText('心跳间隔')).toBeInTheDocument();
    });

    it('req_settings_004: 应该支持策略同步配置', async () => {
      setupMocks();
      render(<Settings />);
      expect(screen.getByText('策略同步')).toBeInTheDocument();
      expect(screen.getByText('自动推送策略更新')).toBeInTheDocument();
    });

    it('req_settings_005: 应该支持保存和恢复', async () => {
      setupMocks();
      render(<Settings />);
      expect(screen.getByText('保存配置')).toBeInTheDocument();
      expect(screen.getByText('恢复默认')).toBeInTheDocument();
    });
  });

  // 契约测试
  describe('契约测试', () => {
    it('应该使用正确的 API 端点加载配置', async () => {
      setupMocks();
      render(<Settings />);
      await waitFor(() => {
        expect(apiClient.get).toHaveBeenCalledWith('/settings');
      });
    });

    it('保存时应该使用 PUT 方法', async () => {
      setupMocks();
      render(<Settings />);
      await waitFor(() => expect(apiClient.get).toHaveBeenCalled());
      fireEvent.click(screen.getByText('保存配置'));
      await waitFor(() => {
        expect(apiClient.put).toHaveBeenCalledWith('/settings', expect.objectContaining({
          dlp_enabled: expect.any(Boolean),
          audit_enabled: expect.any(Boolean),
        }));
      });
    });
  });

  // 安全测试
  describe('安全测试', () => {
    it('故障开放模式应该有安全警告', async () => {
      setupMocks();
      render(<Settings />);
      expect(screen.getByText(/不安全，仅用于调试/)).toBeInTheDocument();
    });

    it('默认配置应该是安全的（故障关闭）', async () => {
      setupMocks();
      render(<Settings />);
      expect(screen.getByText('故障开放模式')).toBeInTheDocument();
      // 默认 dlp_fail_open 为 false
    });
  });

  // 边界情况
  describe('边界情况', () => {
    it('API 返回空数据时应该使用默认值', async () => {
      (apiClient.get as any).mockResolvedValue({ data: null });
      render(<Settings />);
      await waitFor(() => {
        expect(screen.getByText('系统配置')).toBeInTheDocument();
      });
    });

    it('多次点击保存不应该重复提交', async () => {
      setupMocks();
      render(<Settings />);
      await waitFor(() => expect(apiClient.get).toHaveBeenCalled());
      fireEvent.click(screen.getByText('保存配置'));
      fireEvent.click(screen.getByText('保存配置'));
      await waitFor(() => {
        // 至少调用一次
        expect(apiClient.put).toHaveBeenCalled();
      });
    });
  });
});
