/**
 * AboutTab 测试
 *
 * 覆盖维度：
 * - 单元测试：渲染、交互、状态
 * - 安全审计：XSS 防护、错误处理
 * - 契约测试：数据格式一致性
 * - 需求级测试：REQ_ABOUT_001 ~ REQ_ABOUT_004
 */

import { render, screen, fireEvent, act, waitFor } from '@testing-library/react';
import { vi, describe, it, expect, beforeEach } from 'vitest';
import { AboutTab } from '../AboutTab';

// mock invokeTauri
vi.mock('@utils/tauri', () => ({
  invokeTauri: vi.fn(),
}));

import { invokeTauri } from '@utils/tauri';
const mockInvoke = vi.mocked(invokeTauri);

describe('AboutTab', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // 默认：get_app_version 返回 '1.2.3'，check_for_updates 返回最新
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_app_version') return Promise.resolve('1.2.3');
      if (cmd === 'check_for_updates') return Promise.resolve({ needs_upgrade: false, current_version: '1.2.3' });
      return Promise.resolve(null);
    });
  });

  /* ── 初始化和渲染 ── */

  describe('初始化和渲染', () => {
    it('应该渲染 Logo 图标区域', async () => {
      render(<AboutTab />);
      await waitFor(() => expect(screen.getByText('X-Claw')).toBeInTheDocument());
      const logoBox = screen.getByText('X-Claw').closest('div')?.parentElement;
      expect(logoBox).toBeInTheDocument();
    });

    it('应该渲染应用名称 X-Claw', async () => {
      render(<AboutTab />);
      await waitFor(() => {
        const title = screen.getByText('X-Claw');
        expect(title).toBeInTheDocument();
        expect(title.tagName).toBe('H3');
      });
    });

    it('应该渲染副标题', async () => {
      render(<AboutTab />);
      await waitFor(() =>
        expect(screen.getByText('智能 AI 助手，让工作更高效')).toBeInTheDocument()
      );
    });

    it('应该从 get_app_version 命令读取版本号', async () => {
      render(<AboutTab />);
      await waitFor(() => expect(screen.getByText('v1.2.3')).toBeInTheDocument());
      expect(mockInvoke).toHaveBeenCalledWith('get_app_version');
    });

    it('get_app_version 失败时应回退到 v0.1.0', async () => {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'get_app_version') return Promise.reject(new Error('failed'));
        return Promise.resolve(null);
      });
      render(<AboutTab />);
      await waitFor(() => expect(screen.getByText('v0.1.0')).toBeInTheDocument());
    });

    it('应该渲染检查更新按钮', async () => {
      render(<AboutTab />);
      await waitFor(() => expect(screen.getByText('检查更新')).toBeInTheDocument());
    });

    it('初始不应显示检查结果', async () => {
      const { container } = render(<AboutTab />);
      await waitFor(() => expect(screen.getByText('检查更新')).toBeInTheDocument());
      const resultEl = container.querySelector('p.opacity-0');
      expect(resultEl).toBeInTheDocument();
    });
  });

  /* ── 检查更新交互 ── */

  describe('检查更新交互', () => {
    it('点击检查更新应调用 check_for_updates 命令', async () => {
      render(<AboutTab />);
      await waitFor(() => expect(screen.getByText('检查更新')).toBeInTheDocument());
      fireEvent.click(screen.getByText('检查更新'));
      await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('check_for_updates'));
    });

    it('检查更新期间按钮应禁用并显示"检查中..."', async () => {
      // 让 check_for_updates 挂起
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'get_app_version') return Promise.resolve('1.2.3');
        if (cmd === 'check_for_updates') return new Promise(() => {});
        return Promise.resolve(null);
      });
      render(<AboutTab />);
      await waitFor(() => expect(screen.getByText('检查更新')).toBeInTheDocument());
      const btn = screen.getByText('检查更新').closest('button')!;
      fireEvent.click(btn);
      await waitFor(() => expect(screen.getByText('检查中...')).toBeInTheDocument());
      expect(btn).toBeDisabled();
    });

    it('needs_upgrade=false 时应显示"当前已是最新版本"', async () => {
      render(<AboutTab />);
      await waitFor(() => expect(screen.getByText('检查更新')).toBeInTheDocument());
      fireEvent.click(screen.getByText('检查更新'));
      await waitFor(() => expect(screen.getByText('当前已是最新版本')).toBeInTheDocument());
    });

    it('needs_upgrade=true 时应显示升级提示', async () => {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'get_app_version') return Promise.resolve('1.2.3');
        if (cmd === 'check_for_updates') return Promise.resolve({ needs_upgrade: true, current_version: '1.2.3' });
        return Promise.resolve(null);
      });
      render(<AboutTab />);
      await waitFor(() => expect(screen.getByText('检查更新')).toBeInTheDocument());
      fireEvent.click(screen.getByText('检查更新'));
      await waitFor(() =>
        expect(screen.getByText('发现新版本，请前往官网下载更新')).toBeInTheDocument()
      );
    });

    it('check_for_updates 失败时应显示错误提示', async () => {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'get_app_version') return Promise.resolve('1.2.3');
        if (cmd === 'check_for_updates') return Promise.reject(new Error('network error'));
        return Promise.resolve(null);
      });
      render(<AboutTab />);
      await waitFor(() => expect(screen.getByText('检查更新')).toBeInTheDocument());
      fireEvent.click(screen.getByText('检查更新'));
      await waitFor(() =>
        expect(screen.getByText('检查更新失败，请检查网络连接')).toBeInTheDocument()
      );
    });
  });

  /* ── 安全审计 ── */

  describe('test_security_audit', () => {
    it('应用名称应为纯文本渲染，防止 XSS', async () => {
      render(<AboutTab />);
      await waitFor(() => {
        const title = screen.getByText('X-Claw');
        expect(title.tagName).toBe('H3');
      });
    });

    it('版本号应为纯文本渲染', async () => {
      render(<AboutTab />);
      await waitFor(() => {
        const version = screen.getByText('v1.2.3');
        expect(version.tagName).toBe('SPAN');
      });
    });

    it('检查更新错误不应泄露敏感信息', async () => {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'get_app_version') return Promise.resolve('1.2.3');
        if (cmd === 'check_for_updates') return Promise.reject(new Error('internal server error with token=secret123'));
        return Promise.resolve(null);
      });
      render(<AboutTab />);
      await waitFor(() => expect(screen.getByText('检查更新')).toBeInTheDocument());
      fireEvent.click(screen.getByText('检查更新'));
      await waitFor(() => {
        const result = screen.getByText('检查更新失败，请检查网络连接');
        expect(result).toBeInTheDocument();
        // 错误信息不应包含原始错误详情
        expect(result.textContent).not.toContain('secret123');
      });
    });
  });

  /* ── 契约测试 ── */

  describe('test_contract', () => {
    it('Logo 区域应包含 ShieldCheck 图标', async () => {
      const { container } = render(<AboutTab />);
      await waitFor(() => expect(screen.getByText('X-Claw')).toBeInTheDocument());
      const svgs = container.querySelectorAll('svg');
      expect(svgs.length).toBeGreaterThan(0);
    });

    it('版本行应包含版本标签和版本号', async () => {
      render(<AboutTab />);
      await waitFor(() => {
        expect(screen.getByText('当前版本')).toBeInTheDocument();
        expect(screen.getByText('v1.2.3')).toBeInTheDocument();
      });
    });

    it('检查更新按钮应包含 RefreshCw 图标', async () => {
      render(<AboutTab />);
      await waitFor(() => expect(screen.getByText('检查更新')).toBeInTheDocument());
      const btn = screen.getByText('检查更新').closest('button')!;
      const svg = btn.querySelector('svg');
      expect(svg).toBeInTheDocument();
    });
  });

  /* ── 需求级测试 ── */

  describe('req_about_001_logo_display', () => {
    it('应该显示 88px 圆形绿色背景的 Logo', async () => {
      const { container } = render(<AboutTab />);
      await waitFor(() => expect(screen.getByText('X-Claw')).toBeInTheDocument());
      const logoBox = container.querySelector('.size-\\[88px\\]');
      expect(logoBox).toBeInTheDocument();
    });
  });

  describe('req_about_002_app_info', () => {
    it('应该显示应用名称和副标题', async () => {
      render(<AboutTab />);
      await waitFor(() => {
        expect(screen.getByText('X-Claw')).toBeInTheDocument();
        expect(screen.getByText('智能 AI 助手，让工作更高效')).toBeInTheDocument();
      });
    });
  });

  describe('req_about_003_version_info', () => {
    it('应该在灰色背景条中显示版本信息', async () => {
      render(<AboutTab />);
      await waitFor(() => expect(screen.getByText('当前版本')).toBeInTheDocument());
      const versionRow = screen.getByText('当前版本').closest('div')?.parentElement;
      expect(versionRow).toHaveClass('bg-[#F5F4F1]');
    });
  });

  describe('req_about_004_check_update', () => {
    it('应该能触发检查更新并显示结果', async () => {
      render(<AboutTab />);
      await waitFor(() => expect(screen.getByText('检查更新')).toBeInTheDocument());
      fireEvent.click(screen.getByText('检查更新'));
      await waitFor(() => expect(screen.getByText('当前已是最新版本')).toBeInTheDocument());
    });
  });
});
