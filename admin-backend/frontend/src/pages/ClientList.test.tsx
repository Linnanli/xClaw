import React from 'react';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { message } from 'antd';
import { ClientList } from './ClientList';

vi.mock('../api/client', () => ({
  apiClient: {
    get: vi.fn(),
    delete: vi.fn(),
  },
}));

import { apiClient } from '../api/client';

const mockClients = [
  {
    id: 'c1', user_id: 'u1', username: 'alice', client_name: 'Alice MacBook',
    version: '1.2.0', os: 'macOS', ip_address: '192.168.1.10',
    last_activity: new Date().toISOString(), online: true,
    policy_version: 'v3', registered_at: new Date().toISOString(), updated_at: new Date().toISOString(),
  },
  {
    id: 'c2', user_id: 'u2', username: 'bob', client_name: 'Bob Windows',
    version: '1.1.0', os: 'Windows 11', ip_address: '192.168.1.20',
    last_activity: new Date(Date.now() - 86400000).toISOString(), online: false,
    policy_version: null, registered_at: new Date().toISOString(), updated_at: new Date().toISOString(),
  },
];

const mockStats = { total: 2, online: 1, offline: 1, by_os: [{ os: 'macOS', count: 1 }, { os: 'Windows 11', count: 1 }], by_version: [{ version: '1.2.0', count: 1 }, { version: '1.1.0', count: 1 }] };

function setupMocks(overrides?: { clients?: any[]; stats?: any; error?: boolean }) {
  (apiClient.get as any).mockImplementation((url: string) => {
    if (overrides?.error) return Promise.reject({ response: { data: { error: '服务器错误' } } });
    if (url.includes('/clients/stats')) return Promise.resolve({ data: overrides?.stats ?? mockStats });
    if (url.includes('/clients')) return Promise.resolve({ data: { clients: overrides?.clients ?? mockClients } });
    return Promise.resolve({ data: {} });
  });
  (apiClient.delete as any).mockResolvedValue({ data: { message: '删除成功' } });
}

describe('ClientList', () => {
  beforeEach(() => vi.clearAllMocks());

  // 单元测试 - 正常路径
  describe('单元测试 - 正常路径', () => {
    it('应该渲染页面标题', async () => {
      setupMocks();
      render(<ClientList />);
      expect(screen.getByText('客户端管理')).toBeInTheDocument();
    });

    it('应该加载并显示客户端列表', async () => {
      setupMocks();
      render(<ClientList />);
      await waitFor(() => {
        expect(screen.getByText('Alice MacBook')).toBeInTheDocument();
        expect(screen.getByText('Bob Windows')).toBeInTheDocument();
      });
    });

    it('应该显示在线/离线状态', async () => {
      setupMocks();
      render(<ClientList />);
      await waitFor(() => {
        const onlineDots = document.querySelectorAll('.client-online-dot.online');
        const offlineDots = document.querySelectorAll('.client-online-dot.offline');
        expect(onlineDots.length).toBeGreaterThan(0);
        expect(offlineDots.length).toBeGreaterThan(0);
      });
    });

    it('应该显示统计卡片', async () => {
      setupMocks();
      render(<ClientList />);
      await waitFor(() => {
        expect(screen.getByText('总客户端')).toBeInTheDocument();
      });
    });

    it('应该显示版本标签', async () => {
      setupMocks();
      render(<ClientList />);
      await waitFor(() => {
        expect(screen.getByText('1.2.0')).toBeInTheDocument();
        expect(screen.getByText('1.1.0')).toBeInTheDocument();
      });
    });

    it('应该显示策略版本', async () => {
      setupMocks();
      render(<ClientList />);
      await waitFor(() => {
        expect(screen.getByText('v3')).toBeInTheDocument();
        expect(screen.getByText('未同步')).toBeInTheDocument();
      });
    });

    it('应该显示 IP 地址', async () => {
      setupMocks();
      render(<ClientList />);
      await waitFor(() => {
        expect(screen.getByText('192.168.1.10')).toBeInTheDocument();
      });
    });
  });

  // 单元测试 - 失败路径
  describe('单元测试 - 失败路径', () => {
    it('API 失败时应该显示错误消息', async () => {
      setupMocks({ error: true });
      const spy = vi.spyOn(message, 'error');
      render(<ClientList />);
      await waitFor(() => expect(spy).toHaveBeenCalledWith('服务器错误'));
    });

    it('空数据时应该显示空状态', async () => {
      setupMocks({ clients: [] });
      render(<ClientList />);
      await waitFor(() => {
        expect(screen.getByText(/暂无已注册的客户端/)).toBeInTheDocument();
      });
    });
  });

  // 集成测试
  describe('集成测试', () => {
    it('刷新按钮应该重新加载数据', async () => {
      setupMocks();
      render(<ClientList />);
      await waitFor(() => expect(apiClient.get).toHaveBeenCalled());
      const callCount = (apiClient.get as any).mock.calls.length;
      fireEvent.click(screen.getByText('刷新'));
      await waitFor(() => expect((apiClient.get as any).mock.calls.length).toBeGreaterThan(callCount));
    });

    it('搜索应该触发 API 调用', async () => {
      setupMocks();
      render(<ClientList />);
      await waitFor(() => expect(apiClient.get).toHaveBeenCalled());
      fireEvent.change(screen.getByPlaceholderText('搜索用户名、客户端名或 IP'), { target: { value: 'alice' } });
      await waitFor(() => {
        const calls = (apiClient.get as any).mock.calls;
        const lastCall = calls.filter((c: any) => c[0] === '/clients').pop();
        expect(lastCall?.[1]?.params?.search).toBe('alice');
      });
    });

    it('删除客户端应该调用 API 并刷新', async () => {
      setupMocks();
      render(<ClientList />);
      await waitFor(() => expect(screen.getByText('Alice MacBook')).toBeInTheDocument());
      const deleteButtons = screen.getAllByText('删除');
      fireEvent.click(deleteButtons[0]);
      // Popconfirm 使用 okText="确定"
      await waitFor(() => expect(screen.getByText('确认删除')).toBeInTheDocument());
      const okButton = document.querySelector('.ant-popconfirm .ant-btn-primary');
      if (okButton) fireEvent.click(okButton);
      await waitFor(() => expect(apiClient.delete).toHaveBeenCalledWith('/clients/c1'));
    });
  });

  // 需求级测试
  describe('需求级测试', () => {
    it('req_client_001: 应该展示客户端列表', async () => {
      setupMocks();
      render(<ClientList />);
      await waitFor(() => {
        expect(screen.getByText('Alice MacBook')).toBeInTheDocument();
      });
    });

    it('req_client_002: 应该支持搜索筛选', async () => {
      setupMocks();
      render(<ClientList />);
      expect(screen.getByPlaceholderText('搜索用户名、客户端名或 IP')).toBeInTheDocument();
    });

    it('req_client_003: 应该支持在线状态筛选', async () => {
      setupMocks();
      render(<ClientList />);
      expect(screen.getByText('在线状态')).toBeInTheDocument();
    });

    it('req_client_004: 应该显示统计概览', async () => {
      setupMocks();
      render(<ClientList />);
      await waitFor(() => {
        expect(screen.getByText('总客户端')).toBeInTheDocument();
      });
    });

    it('req_client_005: 应该支持删除客户端记录', async () => {
      setupMocks();
      render(<ClientList />);
      await waitFor(() => {
        expect(screen.getAllByText('删除').length).toBeGreaterThan(0);
      });
    });
  });

  // 契约测试
  describe('契约测试', () => {
    it('应该使用正确的 API 端点', async () => {
      setupMocks();
      render(<ClientList />);
      await waitFor(() => {
        expect(apiClient.get).toHaveBeenCalledWith('/clients', expect.any(Object));
        expect(apiClient.get).toHaveBeenCalledWith('/clients/stats');
      });
    });
  });

  // 边界情况
  describe('边界情况', () => {
    it('应该处理缺失字段的客户端数据', async () => {
      const partialClients = [{ id: 'c1', last_activity: new Date().toISOString(), online: false, registered_at: new Date().toISOString(), updated_at: new Date().toISOString() }];
      setupMocks({ clients: partialClients });
      render(<ClientList />);
      await waitFor(() => expect(screen.getByText('客户端管理')).toBeInTheDocument());
    });

    it('统计加载失败不应该影响列表', async () => {
      (apiClient.get as any).mockImplementation((url: string) => {
        if (url.includes('/stats')) return Promise.reject(new Error('fail'));
        return Promise.resolve({ data: { clients: mockClients } });
      });
      render(<ClientList />);
      await waitFor(() => expect(screen.getByText('Alice MacBook')).toBeInTheDocument());
    });
  });
});
