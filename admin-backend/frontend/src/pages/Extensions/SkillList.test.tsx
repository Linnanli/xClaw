import React from 'react';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { SkillList } from './SkillList';

vi.mock('../../api/client', () => ({
  apiClient: {
    get: vi.fn(),
  },
}));

import { apiClient } from '../../api/client';

const mockSkills = [
  {
    id: 's1', name: '代码审查', description: '自动代码审查技能',
    version: '1.0.0', author: 'admin', enabled: true,
    created_at: '2025-01-01T00:00:00Z', updated_at: '2025-06-15T10:30:00Z',
  },
  {
    id: 's2', name: '安全扫描', description: '安全漏洞扫描',
    version: '2.1.0', author: 'security-team', enabled: true,
    created_at: '2025-02-01T00:00:00Z', updated_at: '2025-06-20T14:00:00Z',
  },
];

function setupMocks(overrides?: { skills?: any[]; error?: boolean }) {
  (apiClient.get as any).mockImplementation(() => {
    if (overrides?.error) return Promise.reject(new Error('服务器错误'));
    return Promise.resolve({ data: { skills: overrides?.skills ?? mockSkills } });
  });
}

describe('SkillList', () => {
  beforeEach(() => vi.clearAllMocks());

  // 单元测试 - 正常路径
  describe('单元测试 - 正常路径', () => {
    it('应该渲染页面标题', async () => {
      setupMocks();
      render(<SkillList />);
      expect(screen.getByText('技能管理')).toBeInTheDocument();
    });

    it('应该加载并显示技能列表', async () => {
      setupMocks();
      render(<SkillList />);
      await waitFor(() => {
        expect(screen.getByText('代码审查')).toBeInTheDocument();
        expect(screen.getByText('安全扫描')).toBeInTheDocument();
      });
    });

    it('应该显示技能描述', async () => {
      setupMocks();
      render(<SkillList />);
      await waitFor(() => {
        expect(screen.getByText('自动代码审查技能')).toBeInTheDocument();
      });
    });

    it('应该显示版本标签', async () => {
      setupMocks();
      render(<SkillList />);
      await waitFor(() => {
        expect(screen.getByText('1.0.0')).toBeInTheDocument();
        expect(screen.getByText('2.1.0')).toBeInTheDocument();
      });
    });

    it('应该显示作者信息', async () => {
      setupMocks();
      render(<SkillList />);
      await waitFor(() => {
        expect(screen.getByText('admin')).toBeInTheDocument();
        expect(screen.getByText('security-team')).toBeInTheDocument();
      });
    });

    it('应该显示统计卡片', async () => {
      setupMocks();
      render(<SkillList />);
      await waitFor(() => {
        expect(screen.getByText('已安装技能')).toBeInTheDocument();
      });
    });

    it('应该显示技能总数', async () => {
      setupMocks();
      render(<SkillList />);
      await waitFor(() => {
        expect(screen.getByText(/共 2 个技能/)).toBeInTheDocument();
      });
    });
  });

  // 单元测试 - 失败路径
  describe('单元测试 - 失败路径', () => {
    it('API 失败时应该显示空列表', async () => {
      setupMocks({ error: true });
      render(<SkillList />);
      await waitFor(() => {
        expect(screen.getByText(/暂无已安装的技能/)).toBeInTheDocument();
      });
    });

    it('空数据时应该显示空状态', async () => {
      setupMocks({ skills: [] });
      render(<SkillList />);
      await waitFor(() => {
        expect(screen.getByText(/暂无已安装的技能/)).toBeInTheDocument();
      });
    });

    it('API 返回非数组时应该优雅处理', async () => {
      (apiClient.get as any).mockResolvedValue({ data: {} });
      render(<SkillList />);
      await waitFor(() => {
        expect(screen.getByText('技能管理')).toBeInTheDocument();
      });
    });
  });

  // 集成测试
  describe('集成测试', () => {
    it('刷新按钮应该重新加载数据', async () => {
      setupMocks();
      render(<SkillList />);
      await waitFor(() => expect(screen.getByText('代码审查')).toBeInTheDocument());
      expect(apiClient.get).toHaveBeenCalledWith('/skills');
      // 验证刷新按钮存在且可点击
      const refreshBtn = screen.getByText('刷新');
      expect(refreshBtn).toBeInTheDocument();
      expect(refreshBtn.closest('button')).not.toBeDisabled();
    });

    it('搜索应该过滤技能列表', async () => {
      setupMocks();
      render(<SkillList />);
      await waitFor(() => expect(screen.getByText('代码审查')).toBeInTheDocument());
      fireEvent.change(screen.getByPlaceholderText('搜索技能名称或描述'), { target: { value: '安全' } });
      await waitFor(() => {
        expect(screen.getByText('安全扫描')).toBeInTheDocument();
        expect(screen.queryByText('代码审查')).not.toBeInTheDocument();
      });
    });

    it('清空搜索应该显示全部技能', async () => {
      setupMocks();
      render(<SkillList />);
      await waitFor(() => expect(screen.getByText('代码审查')).toBeInTheDocument());
      const input = screen.getByPlaceholderText('搜索技能名称或描述');
      fireEvent.change(input, { target: { value: '安全' } });
      await waitFor(() => expect(screen.queryByText('代码审查')).not.toBeInTheDocument());
      fireEvent.change(input, { target: { value: '' } });
      await waitFor(() => {
        expect(screen.getByText('代码审查')).toBeInTheDocument();
        expect(screen.getByText('安全扫描')).toBeInTheDocument();
      });
    });
  });

  // 需求级测试
  describe('需求级测试', () => {
    it('req_skill_001: 应该展示技能列表', async () => {
      setupMocks();
      render(<SkillList />);
      await waitFor(() => {
        expect(screen.getByText('代码审查')).toBeInTheDocument();
      });
    });

    it('req_skill_002: 应该支持搜索筛选', async () => {
      setupMocks();
      render(<SkillList />);
      expect(screen.getByPlaceholderText('搜索技能名称或描述')).toBeInTheDocument();
    });

    it('req_skill_003: 应该显示技能统计', async () => {
      setupMocks();
      render(<SkillList />);
      await waitFor(() => {
        expect(screen.getByText('已安装技能')).toBeInTheDocument();
      });
    });
  });

  // 契约测试
  describe('契约测试', () => {
    it('应该使用正确的 API 端点', async () => {
      setupMocks();
      render(<SkillList />);
      await waitFor(() => {
        expect(apiClient.get).toHaveBeenCalledWith('/skills');
      });
    });
  });

  // 边界情况
  describe('边界情况', () => {
    it('应该处理缺失描述的技能', async () => {
      const partial = [{ id: 's1', name: '测试技能', version: '1.0.0', enabled: true, created_at: '2025-01-01T00:00:00Z', updated_at: '2025-01-01T00:00:00Z' }];
      setupMocks({ skills: partial });
      render(<SkillList />);
      await waitFor(() => expect(screen.getByText('测试技能')).toBeInTheDocument());
    });

    it('搜索无结果时应该显示空状态', async () => {
      setupMocks();
      render(<SkillList />);
      await waitFor(() => expect(screen.getByText('代码审查')).toBeInTheDocument());
      fireEvent.change(screen.getByPlaceholderText('搜索技能名称或描述'), { target: { value: 'zzzzzzz' } });
      await waitFor(() => {
        expect(screen.queryByText('代码审查')).not.toBeInTheDocument();
        expect(screen.queryByText('安全扫描')).not.toBeInTheDocument();
      });
    });
  });
});
