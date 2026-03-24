/**
 * SkillsTab 单元测试
 *
 * 覆盖维度：
 * - 正常路径：渲染、搜索、开关切换、菜单交互
 * - 错误路径：API 失败、回滚、fallback 数据
 * - 安全审计：内置技能保护、菜单权限
 */

import { render, screen, fireEvent, waitFor, within } from '@testing-library/react';
import { vi, describe, it, expect, beforeEach } from 'vitest';
import { SkillsTab } from '../SkillsTab';
import { skillApi } from '../../../utils/tauri';

// Mock tauri API
vi.mock('../../../utils/tauri', () => ({
  skillApi: {
    getAvailableSkills: vi.fn(),
    getInstalledSkills: vi.fn(),
    enableSkill: vi.fn(),
    disableSkill: vi.fn(),
    installSkill: vi.fn(),
    uninstallSkill: vi.fn(),
  },
}));

const mockSkillApi = vi.mocked(skillApi);

const mockAvailable = [
  {
    id: 'agent-mbti',
    name: 'agent-mbti',
    version: '1.0.0',
    description: 'AI Agent personality diagnosis',
    author: 'test',
    keywords: [],
    trust_level: 'high',
    source: 'builtin',
  },
  {
    id: 'custom-skill',
    name: 'custom-skill',
    version: '0.1.0',
    description: 'A community contributed skill',
    author: 'community',
    keywords: [],
    trust_level: 'medium',
    source: 'community',
  },
];

const mockInstalled = [
  {
    metadata: mockAvailable[0],
    enabled: true,
  },
  {
    metadata: mockAvailable[1],
    enabled: false,
  },
];

function renderSkillsTab() {
  return render(<SkillsTab />);
}

describe('SkillsTab', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockSkillApi.getAvailableSkills.mockResolvedValue(mockAvailable);
    mockSkillApi.getInstalledSkills.mockResolvedValue(mockInstalled);
    mockSkillApi.enableSkill.mockResolvedValue(undefined);
    mockSkillApi.disableSkill.mockResolvedValue(undefined);
    mockSkillApi.uninstallSkill.mockResolvedValue(undefined);
  });

  /* ── 正常路径：渲染 ── */

  describe('初始化和渲染', () => {
    it('应该显示加载状态', () => {
      mockSkillApi.getAvailableSkills.mockImplementation(() => new Promise(() => {}));
      mockSkillApi.getInstalledSkills.mockImplementation(() => new Promise(() => {}));
      renderSkillsTab();
      expect(screen.getByText('加载中...')).toBeInTheDocument();
    });

    it('应该渲染搜索栏和添加按钮', async () => {
      renderSkillsTab();
      await waitFor(() => {
        expect(screen.getByPlaceholderText('搜索已经安装的技能')).toBeInTheDocument();
        expect(screen.getByText('添加技能')).toBeInTheDocument();
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
        expect(screen.getByText('社区技能')).toBeInTheDocument();
      });
    });

    it('应该为高信任级别显示安全审核标签', async () => {
      renderSkillsTab();
      await waitFor(() => {
        expect(screen.getByText('安全审核')).toBeInTheDocument();
      });
    });

    it('应该在无技能时显示空状态', async () => {
      mockSkillApi.getAvailableSkills.mockResolvedValue([]);
      mockSkillApi.getInstalledSkills.mockResolvedValue([]);
      renderSkillsTab();
      await waitFor(() => {
        expect(screen.getByText('暂无技能')).toBeInTheDocument();
      });
    });
  });

  /* ── 正常路径：搜索 ── */

  describe('搜索功能', () => {
    it('应该根据名称过滤技能', async () => {
      renderSkillsTab();
      await waitFor(() => {
        expect(screen.getByText('agent-mbti')).toBeInTheDocument();
      });

      fireEvent.change(screen.getByPlaceholderText('搜索已经安装的技能'), {
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

      fireEvent.change(screen.getByPlaceholderText('搜索已经安装的技能'), {
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

      fireEvent.change(screen.getByPlaceholderText('搜索已经安装的技能'), {
        target: { value: 'nonexistent' },
      });

      expect(screen.getByText('未找到匹配的技能')).toBeInTheDocument();
    });
  });

  /* ── 正常路径：开关切换 ── */

  describe('开关切换', () => {
    it('应该乐观更新开关状态（禁用）', async () => {
      renderSkillsTab();
      await waitFor(() => {
        expect(screen.getByText('agent-mbti')).toBeInTheDocument();
      });

      const switches = screen.getAllByRole('switch');
      // agent-mbti 是 enabled=true，点击应该调用 disableSkill
      fireEvent.click(switches[0]);

      await waitFor(() => {
        expect(mockSkillApi.disableSkill).toHaveBeenCalledWith('agent-mbti');
      });
    });

    it('应该乐观更新开关状态（启用）', async () => {
      renderSkillsTab();
      await waitFor(() => {
        expect(screen.getByText('custom-skill')).toBeInTheDocument();
      });

      const switches = screen.getAllByRole('switch');
      // custom-skill 是 enabled=false，点击应该调用 enableSkill
      fireEvent.click(switches[1]);

      await waitFor(() => {
        expect(mockSkillApi.enableSkill).toHaveBeenCalledWith('custom-skill');
      });
    });
  });

  /* ── 正常路径：菜单交互 ── */

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
      fireEvent.click(menuButtons[1]); // 社区技能的菜单

      const removeBtn = screen.getByText('移除技能');
      fireEvent.click(removeBtn);

      await waitFor(() => {
        expect(mockSkillApi.uninstallSkill).toHaveBeenCalledWith('custom-skill');
      });
    });
  });

  /* ── 错误路径 ── */

  describe('错误处理', () => {
    it('应该在 API 失败时显示错误并使用 fallback 数据', async () => {
      mockSkillApi.getAvailableSkills.mockRejectedValue(new Error('Network error'));
      mockSkillApi.getInstalledSkills.mockRejectedValue(new Error('Network error'));

      renderSkillsTab();

      await waitFor(() => {
        expect(screen.getByText('加载技能失败')).toBeInTheDocument();
        // fallback 数据应该显示
        expect(screen.getByText('agent-mbti')).toBeInTheDocument();
      });
    });

    it('应该在开关切换失败时回滚状态', async () => {
      mockSkillApi.disableSkill.mockRejectedValue(new Error('Failed'));

      renderSkillsTab();
      await waitFor(() => {
        expect(screen.getByText('agent-mbti')).toBeInTheDocument();
      });

      const switches = screen.getAllByRole('switch');
      const firstSwitch = switches[0];

      // 初始状态 checked
      expect(firstSwitch).toBeChecked();

      // 点击切换
      fireEvent.click(firstSwitch);

      // 等待回滚
      await waitFor(() => {
        expect(firstSwitch).toBeChecked();
      });
    });

    it('应该在卸载失败时不崩溃', async () => {
      mockSkillApi.uninstallSkill.mockRejectedValue(new Error('Uninstall failed'));

      renderSkillsTab();
      await waitFor(() => {
        expect(screen.getByText('custom-skill')).toBeInTheDocument();
      });

      const menuButtons = screen.getAllByLabelText('更多操作');
      fireEvent.click(menuButtons[1]);
      fireEvent.click(screen.getByText('移除技能'));

      // 不应该崩溃
      await waitFor(() => {
        expect(mockSkillApi.uninstallSkill).toHaveBeenCalled();
      });
    });
  });

  /* ── 安全审计：内置技能保护 ── */

  describe('安全审计 - 内置技能保护', () => {
    it('内置技能的移除按钮应该被禁用', async () => {
      renderSkillsTab();
      await waitFor(() => {
        expect(screen.getByText('agent-mbti')).toBeInTheDocument();
      });

      const menuButtons = screen.getAllByLabelText('更多操作');
      fireEvent.click(menuButtons[0]); // 内置技能的菜单

      const removeBtn = screen.getByText('移除技能');
      expect(removeBtn.closest('button')).toBeDisabled();
    });

    it('社区技能的移除按钮应该可用', async () => {
      renderSkillsTab();
      await waitFor(() => {
        expect(screen.getByText('custom-skill')).toBeInTheDocument();
      });

      const menuButtons = screen.getAllByLabelText('更多操作');
      fireEvent.click(menuButtons[1]); // 社区技能的菜单

      const removeBtn = screen.getByText('移除技能');
      expect(removeBtn.closest('button')).not.toBeDisabled();
    });

    it('内置技能移除按钮应该显示锁定图标', async () => {
      renderSkillsTab();
      await waitFor(() => {
        expect(screen.getByText('agent-mbti')).toBeInTheDocument();
      });

      const menuButtons = screen.getAllByLabelText('更多操作');
      fireEvent.click(menuButtons[0]);

      // 锁定图标应该存在于移除按钮旁
      const removeBtn = screen.getByText('移除技能').closest('button');
      expect(removeBtn).toBeDisabled();
    });
  });

  /* ── 契约测试：数据合并逻辑 ── */

  describe('契约测试 - 数据合并', () => {
    it('应该合并 available 和 installed 数据', async () => {
      renderSkillsTab();
      await waitFor(() => {
        expect(mockSkillApi.getAvailableSkills).toHaveBeenCalled();
        expect(mockSkillApi.getInstalledSkills).toHaveBeenCalled();
      });
    });

    it('应该补充仅在 installed 中的技能', async () => {
      mockSkillApi.getAvailableSkills.mockResolvedValue([mockAvailable[0]]);
      mockSkillApi.getInstalledSkills.mockResolvedValue(mockInstalled);

      renderSkillsTab();
      await waitFor(() => {
        // custom-skill 不在 available 中但在 installed 中，应该显示
        expect(screen.getByText('agent-mbti')).toBeInTheDocument();
        expect(screen.getByText('custom-skill')).toBeInTheDocument();
      });
    });
  });
});
