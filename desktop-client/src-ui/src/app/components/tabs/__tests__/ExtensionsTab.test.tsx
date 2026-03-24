/**
 * ExtensionsTab 单元测试
 *
 * 覆盖维度：
 * - 正常路径：渲染、搜索、安装/卸载、开关切换、菜单交互、Setup 模态框
 * - 错误路径：API 失败、回滚、安装失败
 * - 契约测试：数据合并、Setup schema 格式
 */

import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { vi, describe, it, expect, beforeEach } from 'vitest';
import { ExtensionsTab } from '../ExtensionsTab';
import { extensionApi, extensionSetupApi } from '../../../utils/tauri';

// Mock tauri API
vi.mock('../../../utils/tauri', () => ({
  extensionApi: {
    getInstalledExtensions: vi.fn(),
    getAvailableExtensions: vi.fn(),
    installExtension: vi.fn(),
    uninstallExtension: vi.fn(),
    enableExtension: vi.fn(),
    disableExtension: vi.fn(),
  },
  extensionSetupApi: {
    getSetupSchema: vi.fn(),
    submitSetup: vi.fn(),
  },
}));

const mockExtApi = vi.mocked(extensionApi);
const mockSetupApi = vi.mocked(extensionSetupApi);

const mockAvailable = [
  {
    id: 'ext-github',
    name: 'GitHub Integration',
    version: '2.1.0',
    description: 'Connect to GitHub repositories',
    author: 'official',
    tools: ['github_search', 'github_pr'],
  },
  {
    id: 'ext-slack',
    name: 'Slack Notifier',
    version: '1.0.0',
    description: 'Send notifications to Slack',
    author: 'community',
    tools: ['slack_send'],
  },
];

const mockInstalled = [
  {
    metadata: mockAvailable[0],
    enabled: true,
  },
];

function renderExtensionsTab() {
  return render(<ExtensionsTab />);
}

describe('ExtensionsTab', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockExtApi.getAvailableExtensions.mockResolvedValue(mockAvailable);
    mockExtApi.getInstalledExtensions.mockResolvedValue(mockInstalled);
    mockExtApi.enableExtension.mockResolvedValue(undefined);
    mockExtApi.disableExtension.mockResolvedValue(undefined);
    mockExtApi.installExtension.mockResolvedValue(undefined);
    mockExtApi.uninstallExtension.mockResolvedValue(undefined);
  });

  /* ── 正常路径：渲染 ── */

  describe('初始化和渲染', () => {
    it('应该显示加载状态', () => {
      mockExtApi.getAvailableExtensions.mockImplementation(() => new Promise(() => {}));
      mockExtApi.getInstalledExtensions.mockImplementation(() => new Promise(() => {}));
      renderExtensionsTab();
      expect(screen.getByText('加载中...')).toBeInTheDocument();
    });

    it('应该渲染搜索栏和添加按钮', async () => {
      renderExtensionsTab();
      await waitFor(() => {
        expect(screen.getByPlaceholderText('搜索已安装的扩展')).toBeInTheDocument();
        expect(screen.getByText('添加扩展')).toBeInTheDocument();
      });
    });

    it('应该渲染扩展卡片列表', async () => {
      renderExtensionsTab();
      await waitFor(() => {
        expect(screen.getByText('GitHub Integration')).toBeInTheDocument();
        expect(screen.getByText('Slack Notifier')).toBeInTheDocument();
      });
    });

    it('应该显示版本标签', async () => {
      renderExtensionsTab();
      await waitFor(() => {
        expect(screen.getByText('v2.1.0')).toBeInTheDocument();
        expect(screen.getByText('v1.0.0')).toBeInTheDocument();
      });
    });

    it('已安装且启用的扩展应显示已启用标签', async () => {
      renderExtensionsTab();
      await waitFor(() => {
        expect(screen.getByText('已启用')).toBeInTheDocument();
      });
    });

    it('应该在无扩展时显示空状态', async () => {
      mockExtApi.getAvailableExtensions.mockResolvedValue([]);
      mockExtApi.getInstalledExtensions.mockResolvedValue([]);
      renderExtensionsTab();
      await waitFor(() => {
        expect(screen.getByText('暂无扩展')).toBeInTheDocument();
      });
    });
  });

  /* ── 正常路径：搜索 ── */

  describe('搜索功能', () => {
    it('应该根据名称过滤扩展', async () => {
      renderExtensionsTab();
      await waitFor(() => {
        expect(screen.getByText('GitHub Integration')).toBeInTheDocument();
      });

      fireEvent.change(screen.getByPlaceholderText('搜索已安装的扩展'), {
        target: { value: 'Slack' },
      });

      expect(screen.queryByText('GitHub Integration')).not.toBeInTheDocument();
      expect(screen.getByText('Slack Notifier')).toBeInTheDocument();
    });

    it('应该在无匹配时显示空状态', async () => {
      renderExtensionsTab();
      await waitFor(() => {
        expect(screen.getByText('GitHub Integration')).toBeInTheDocument();
      });

      fireEvent.change(screen.getByPlaceholderText('搜索已安装的扩展'), {
        target: { value: 'nonexistent' },
      });

      expect(screen.getByText('未找到匹配的扩展')).toBeInTheDocument();
    });
  });

  /* ── 正常路径：安装/卸载 ── */

  describe('安装和卸载', () => {
    it('未安装的扩展应显示安装按钮', async () => {
      renderExtensionsTab();
      await waitFor(() => {
        expect(screen.getByText('安装')).toBeInTheDocument();
      });
    });

    it('已安装的扩展应显示开关', async () => {
      renderExtensionsTab();
      await waitFor(() => {
        const switches = screen.getAllByRole('switch');
        expect(switches.length).toBeGreaterThanOrEqual(1);
      });
    });

    it('点击安装按钮应调用安装 API', async () => {
      renderExtensionsTab();
      await waitFor(() => {
        expect(screen.getByText('安装')).toBeInTheDocument();
      });

      fireEvent.click(screen.getByText('安装'));

      await waitFor(() => {
        expect(mockExtApi.installExtension).toHaveBeenCalled();
      });
    });

    it('点击卸载应调用卸载 API', async () => {
      renderExtensionsTab();
      await waitFor(() => {
        expect(screen.getByText('GitHub Integration')).toBeInTheDocument();
      });

      // 打开已安装扩展的菜单
      const menuButtons = screen.getAllByLabelText('更多操作');
      fireEvent.click(menuButtons[0]);

      fireEvent.click(screen.getByText('卸载扩展'));

      await waitFor(() => {
        expect(mockExtApi.uninstallExtension).toHaveBeenCalledWith('ext-github');
      });
    });
  });

  /* ── 正常路径：开关切换 ── */

  describe('开关切换', () => {
    it('应该乐观更新开关状态', async () => {
      renderExtensionsTab();
      await waitFor(() => {
        expect(screen.getByText('GitHub Integration')).toBeInTheDocument();
      });

      const switches = screen.getAllByRole('switch');
      fireEvent.click(switches[0]);

      await waitFor(() => {
        expect(mockExtApi.disableExtension).toHaveBeenCalledWith('ext-github');
      });
    });
  });

  /* ── 正常路径：菜单交互 ── */

  describe('菜单交互', () => {
    it('已安装扩展应显示更多菜单', async () => {
      renderExtensionsTab();
      await waitFor(() => {
        expect(screen.getByText('GitHub Integration')).toBeInTheDocument();
      });

      const menuButtons = screen.getAllByLabelText('更多操作');
      fireEvent.click(menuButtons[0]);

      expect(screen.getByText('配置')).toBeInTheDocument();
      expect(screen.getByText('查看详情')).toBeInTheDocument();
      expect(screen.getByText('卸载扩展')).toBeInTheDocument();
    });

    it('未安装扩展不应显示更多菜单按钮', async () => {
      renderExtensionsTab();
      await waitFor(() => {
        expect(screen.getByText('Slack Notifier')).toBeInTheDocument();
      });

      // 只有已安装的扩展有菜单按钮
      const menuButtons = screen.getAllByLabelText('更多操作');
      expect(menuButtons.length).toBe(1); // 只有 GitHub Integration
    });
  });

  /* ── 正常路径：Setup 模态框 ── */

  describe('Setup 模态框', () => {
    it('点击配置应打开 Setup 模态框', async () => {
      mockSetupApi.getSetupSchema.mockResolvedValue({
        name: 'ext-github',
        kind: 'secrets',
        secrets: [
          {
            name: 'api_key',
            prompt: 'GitHub API Key',
            optional: false,
            provided: false,
            auto_generate: false,
          },
        ],
      });

      renderExtensionsTab();
      await waitFor(() => {
        expect(screen.getByText('GitHub Integration')).toBeInTheDocument();
      });

      const menuButtons = screen.getAllByLabelText('更多操作');
      fireEvent.click(menuButtons[0]);
      fireEvent.click(screen.getByText('配置'));

      await waitFor(() => {
        expect(screen.getByText('GitHub API Key')).toBeInTheDocument();
        expect(screen.getByText('保存配置')).toBeInTheDocument();
      });
    });

    it('应该能关闭 Setup 模态框', async () => {
      mockSetupApi.getSetupSchema.mockResolvedValue({
        name: 'ext-github',
        kind: 'secrets',
        secrets: [],
      });

      renderExtensionsTab();
      await waitFor(() => {
        expect(screen.getByText('GitHub Integration')).toBeInTheDocument();
      });

      const menuButtons = screen.getAllByLabelText('更多操作');
      fireEvent.click(menuButtons[0]);
      fireEvent.click(screen.getByText('配置'));

      await waitFor(() => {
        expect(screen.getByText('关闭')).toBeInTheDocument();
      });

      fireEvent.click(screen.getByText('关闭'));

      await waitFor(() => {
        expect(screen.queryByText('保存配置')).not.toBeInTheDocument();
      });
    });

    it('提交配置应调用 submitSetup API', async () => {
      mockSetupApi.getSetupSchema.mockResolvedValue({
        name: 'ext-github',
        kind: 'secrets',
        secrets: [
          {
            name: 'api_key',
            prompt: 'GitHub API Key',
            optional: false,
            provided: false,
            auto_generate: false,
          },
        ],
      });
      mockSetupApi.submitSetup.mockResolvedValue({
        success: true,
        message: '配置成功',
        activated: true,
        auth_url: null,
      });

      renderExtensionsTab();
      await waitFor(() => {
        expect(screen.getByText('GitHub Integration')).toBeInTheDocument();
      });

      const menuButtons = screen.getAllByLabelText('更多操作');
      fireEvent.click(menuButtons[0]);
      fireEvent.click(screen.getByText('配置'));

      await waitFor(() => {
        expect(screen.getByText('保存配置')).toBeInTheDocument();
      });

      // 输入 API key
      const input = screen.getByPlaceholderText('请输入...');
      fireEvent.change(input, { target: { value: 'test-key-123' } });

      fireEvent.click(screen.getByText('保存配置'));

      await waitFor(() => {
        expect(mockSetupApi.submitSetup).toHaveBeenCalledWith('ext-github', {
          api_key: 'test-key-123',
        });
      });
    });

    it('配置失败应显示错误信息', async () => {
      mockSetupApi.getSetupSchema.mockResolvedValue({
        name: 'ext-github',
        kind: 'secrets',
        secrets: [
          {
            name: 'api_key',
            prompt: 'GitHub API Key',
            optional: false,
            provided: false,
            auto_generate: false,
          },
        ],
      });
      mockSetupApi.submitSetup.mockResolvedValue({
        success: false,
        message: 'Invalid API key',
        activated: false,
        auth_url: null,
      });

      renderExtensionsTab();
      await waitFor(() => {
        expect(screen.getByText('GitHub Integration')).toBeInTheDocument();
      });

      const menuButtons = screen.getAllByLabelText('更多操作');
      fireEvent.click(menuButtons[0]);
      fireEvent.click(screen.getByText('配置'));

      await waitFor(() => {
        expect(screen.getByText('保存配置')).toBeInTheDocument();
      });

      fireEvent.click(screen.getByText('保存配置'));

      await waitFor(() => {
        expect(screen.getByText('Invalid API key')).toBeInTheDocument();
      });
    });
  });

  /* ── 错误路径 ── */

  describe('错误处理', () => {
    it('应该在 API 失败时显示错误', async () => {
      mockExtApi.getAvailableExtensions.mockRejectedValue(new Error('Network error'));
      mockExtApi.getInstalledExtensions.mockRejectedValue(new Error('Network error'));

      renderExtensionsTab();

      await waitFor(() => {
        expect(screen.getByText('加载扩展失败')).toBeInTheDocument();
      });
    });

    it('应该在开关切换失败时回滚状态', async () => {
      mockExtApi.disableExtension.mockRejectedValue(new Error('Failed'));

      renderExtensionsTab();
      await waitFor(() => {
        expect(screen.getByText('GitHub Integration')).toBeInTheDocument();
      });

      const switches = screen.getAllByRole('switch');
      const firstSwitch = switches[0];
      expect(firstSwitch).toBeChecked();

      fireEvent.click(firstSwitch);

      await waitFor(() => {
        expect(firstSwitch).toBeChecked(); // 回滚
      });
    });

    it('应该在安装失败时不崩溃', async () => {
      mockExtApi.installExtension.mockRejectedValue(new Error('Install failed'));

      renderExtensionsTab();
      await waitFor(() => {
        expect(screen.getByText('安装')).toBeInTheDocument();
      });

      fireEvent.click(screen.getByText('安装'));

      // 不应该崩溃
      await waitFor(() => {
        expect(mockExtApi.installExtension).toHaveBeenCalled();
      });
    });

    it('Setup schema 加载失败应显示无需配置', async () => {
      mockSetupApi.getSetupSchema.mockRejectedValue(new Error('Schema error'));

      renderExtensionsTab();
      await waitFor(() => {
        expect(screen.getByText('GitHub Integration')).toBeInTheDocument();
      });

      const menuButtons = screen.getAllByLabelText('更多操作');
      fireEvent.click(menuButtons[0]);
      fireEvent.click(screen.getByText('配置'));

      await waitFor(() => {
        expect(screen.getByText('此扩展无需配置')).toBeInTheDocument();
      });
    });
  });

  /* ── 契约测试 ── */

  describe('契约测试 - 数据合并', () => {
    it('应该合并 available 和 installed 数据', async () => {
      renderExtensionsTab();
      await waitFor(() => {
        expect(mockExtApi.getAvailableExtensions).toHaveBeenCalled();
        expect(mockExtApi.getInstalledExtensions).toHaveBeenCalled();
      });
    });

    it('已安装扩展应标记 installed=true', async () => {
      renderExtensionsTab();
      await waitFor(() => {
        // GitHub Integration 已安装，应该有开关而非安装按钮
        const switches = screen.getAllByRole('switch');
        expect(switches.length).toBe(1);
        // Slack Notifier 未安装，应该有安装按钮
        expect(screen.getByText('安装')).toBeInTheDocument();
      });
    });

    it('应该补充仅在 installed 中的扩展', async () => {
      mockExtApi.getAvailableExtensions.mockResolvedValue([mockAvailable[1]]);
      mockExtApi.getInstalledExtensions.mockResolvedValue(mockInstalled);

      renderExtensionsTab();
      await waitFor(() => {
        expect(screen.getByText('GitHub Integration')).toBeInTheDocument();
        expect(screen.getByText('Slack Notifier')).toBeInTheDocument();
      });
    });
  });
});
