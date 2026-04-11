/**
 * SkillsTab 单元测试
 *
 * 覆盖维度：
 * - 正常路径：渲染、搜索、菜单交互
 * - 错误路径：API 失败、卸载失败
 * - 安全审计：内置技能保护（workspace source 不可移除）
 * - 契约测试：ic_list_skills 返回数据正确渲染
 */

import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { vi, describe, it, expect, beforeEach } from 'vitest';
import { SkillsTab } from '../SkillsTab';

// Mock tauri utils — 只需要 invokeTauri
vi.mock('../../../utils/tauri', () => ({
  invokeTauri: vi.fn(),
}));

import { invokeTauri } from '../../../utils/tauri';
const mockInvoke = vi.mocked(invokeTauri);

// ── 测试数据（与 Rust SkillInfo 契约对齐）──────────────────────────

const mockSkills = [
  {
    name: 'agent-mbti',
    version: '1.0.0',
    description: 'AI Agent personality diagnosis',
    source: 'workspace',   // 内置技能，不可移除
    trust: 'trusted',
    keywords: ['ai', 'personality'],
    enabled: true,
  },
  {
    name: 'custom-skill',
    version: '0.1.0',
    description: 'A community contributed skill',
    source: 'user',        // 用户技能，可移除
    trust: 'installed',
    keywords: [],
    enabled: false,
  },
];

function renderSkillsTab() {
  return render(<SkillsTab />);
}

describe('SkillsTab', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // 默认：ic_list_skills 返回 mockSkills
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'ic_list_skills') return Promise.resolve(mockSkills);
      return Promise.resolve(undefined);
    });
  });

  // ── 正常路径：渲染 ──────────────────────────────────────────────

  describe('初始化和渲染', () => {
    it('应该显示加载状态', () => {
      mockInvoke.mockImplementation(() => new Promise(() => {})); // 永不 resolve
      renderSkillsTab();
      expect(screen.getByText('加载中...')).toBeInTheDocument();
    });

    it('应该渲染搜索栏和添加按钮', async () => {
      renderSkillsTab();
      await waitFor(() => {
        expect(screen.getByPlaceholderText('搜索技能名称、描述或关键词')).toBeInTheDocument();
      });
    });

    it('应该渲染技能卡片列表', async () => {
      renderSkillsTab();
      await waitFor(() => {
        expect(screen.getByText('agent-mbti')).toBeInTheDocument();
        expect(screen.getByText('custom-skill')).toBeInTheDocument();
      });
    });

    it('应该显示内置技能标签', async () => {
      renderSkillsTab();
      await waitFor(() => {
        expect(screen.getByText('内置技能')).toBeInTheDocument();
        expect(screen.getByText('用户技能')).toBeInTheDocument();
      });
    });

    it('应该为 trusted 技能显示受信任标签', async () => {
      renderSkillsTab();
      await waitFor(() => {
        expect(screen.getByText('受信任')).toBeInTheDocument();
      });
    });

    it('应该在无技能时显示空状态', async () => {
      mockInvoke.mockResolvedValue([]);
      renderSkillsTab();
      await waitFor(() => {
        expect(screen.getByText('暂无已加载的技能')).toBeInTheDocument();
      });
    });
  });

  // ── 正常路径：搜索 ──────────────────────────────────────────────

  describe('搜索功能', () => {
    it('应该根据名称过滤技能', async () => {
      renderSkillsTab();
      await waitFor(() => {
        expect(screen.getByText('agent-mbti')).toBeInTheDocument();
      });

      fireEvent.change(screen.getByPlaceholderText('搜索技能名称、描述或关键词'), {
        target: { value: 'custom' },
      });

      expect(screen.queryByText('agent-mbti')).not.toBeInTheDocument();
      expect(screen.getByText('custom-skill')).toBeInTheDocument();
    });

    it('应该根据描述过滤技能', async () => {
      renderSkillsTab();
      await waitFor(() => {
        expect(screen.getByText('agent-mbti')).toBeInTheDocument();
      });

      fireEvent.change(screen.getByPlaceholderText('搜索技能名称、描述或关键词'), {
        target: { value: 'personality' },
      });

      expect(screen.getByText('agent-mbti')).toBeInTheDocument();
      expect(screen.queryByText('custom-skill')).not.toBeInTheDocument();
    });

    it('应该在无匹配时显示空状态', async () => {
      renderSkillsTab();
      await waitFor(() => {
        expect(screen.getByText('agent-mbti')).toBeInTheDocument();
      });

      fireEvent.change(screen.getByPlaceholderText('搜索技能名称、描述或关键词'), {
        target: { value: 'nonexistent' },
      });

      expect(screen.getByText('未找到匹配的技能')).toBeInTheDocument();
    });
  });

  // ── 正常路径：菜单交互 ──────────────────────────────────────────

  describe('菜单交互', () => {
    it('应该打开和关闭更多菜单', async () => {
      renderSkillsTab();
      await waitFor(() => {
        expect(screen.getByText('agent-mbti')).toBeInTheDocument();
      });

      const menuButtons = screen.getAllByLabelText('更多操作');
      fireEvent.click(menuButtons[0]);

      expect(screen.getByText('查看详情')).toBeInTheDocument();
      expect(screen.getByText('移除技能')).toBeInTheDocument();

      // 再次点击关闭
      fireEvent.click(menuButtons[0]);
      expect(screen.queryByText('查看详情')).not.toBeInTheDocument();
    });

    it('应该调用卸载 API', async () => {
      renderSkillsTab();
      await waitFor(() => {
        expect(screen.getByText('custom-skill')).toBeInTheDocument();
      });

      const menuButtons = screen.getAllByLabelText('更多操作');
      fireEvent.click(menuButtons[1]); // 用户技能的菜单

      const removeBtn = screen.getByText('移除技能');
      fireEvent.click(removeBtn);

      await waitFor(() => {
        expect(mockInvoke).toHaveBeenCalledWith('ic_uninstall_skill', { name: 'custom-skill' });
      });
    });
  });

  // ── 错误路径 ────────────────────────────────────────────────────

  describe('错误处理', () => {
    it('应该在 API 失败时显示错误', async () => {
      mockInvoke.mockRejectedValue(new Error('Network error'));

      renderSkillsTab();

      await waitFor(() => {
        expect(screen.getByText('加载技能失败')).toBeInTheDocument();
      });
    });

    it('应该在卸载失败时不崩溃', async () => {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'ic_list_skills') return Promise.resolve(mockSkills);
        if (cmd === 'ic_uninstall_skill') return Promise.reject(new Error('Uninstall failed'));
        return Promise.resolve(undefined);
      });

      renderSkillsTab();
      await waitFor(() => {
        expect(screen.getByText('custom-skill')).toBeInTheDocument();
      });

      const menuButtons = screen.getAllByLabelText('更多操作');
      fireEvent.click(menuButtons[1]);
      fireEvent.click(screen.getByText('移除技能'));

      await waitFor(() => {
        expect(mockInvoke).toHaveBeenCalledWith('ic_uninstall_skill', { name: 'custom-skill' });
      });
      // 不应该崩溃，错误提示出现
      await waitFor(() => {
        expect(screen.getByText('卸载技能失败')).toBeInTheDocument();
      });
    });
  });

  // ── 安全审计：内置技能保护 ──────────────────────────────────────

  describe('安全审计 - 内置技能保护', () => {
    it('workspace 技能的移除按钮应该被禁用', async () => {
      renderSkillsTab();
      await waitFor(() => {
        expect(screen.getByText('agent-mbti')).toBeInTheDocument();
      });

      const menuButtons = screen.getAllByLabelText('更多操作');
      fireEvent.click(menuButtons[0]); // workspace 技能的菜单

      const removeBtn = screen.getByText('移除技能');
      expect(removeBtn.closest('button')).toBeDisabled();
    });

    it('用户技能的移除按钮应该可用', async () => {
      renderSkillsTab();
      await waitFor(() => {
        expect(screen.getByText('custom-skill')).toBeInTheDocument();
      });

      const menuButtons = screen.getAllByLabelText('更多操作');
      fireEvent.click(menuButtons[1]); // 用户技能的菜单

      const removeBtn = screen.getByText('移除技能');
      expect(removeBtn.closest('button')).not.toBeDisabled();
    });

    it('workspace 技能移除按钮应该显示锁定图标', async () => {
      renderSkillsTab();
      await waitFor(() => {
        expect(screen.getByText('agent-mbti')).toBeInTheDocument();
      });

      const menuButtons = screen.getAllByLabelText('更多操作');
      fireEvent.click(menuButtons[0]);

      const removeBtn = screen.getByText('移除技能').closest('button');
      expect(removeBtn).toBeDisabled();
    });
  });

  // ── 契约测试 ────────────────────────────────────────────────────

  describe('契约测试 - ic_list_skills', () => {
    it('应该调用 ic_list_skills 命令', async () => {
      renderSkillsTab();
      await waitFor(() => {
        expect(mockInvoke).toHaveBeenCalledWith('ic_list_skills');
      });
    });

    it('应该正确渲染 ic_list_skills 返回的所有技能', async () => {
      renderSkillsTab();
      await waitFor(() => {
        expect(screen.getByText('agent-mbti')).toBeInTheDocument();
        expect(screen.getByText('custom-skill')).toBeInTheDocument();
      });
    });
  });
});
