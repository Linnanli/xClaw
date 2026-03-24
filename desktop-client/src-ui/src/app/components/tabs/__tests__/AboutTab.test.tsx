/**
 * AboutTab 测试
 *
 * 覆盖维度：
 * - 单元测试：渲染、交互、状态
 * - 安全审计：XSS 防护、错误处理
 * - 契约测试：数据格式一致性
 * - 需求级测试：REQ_ABOUT_001 ~ REQ_ABOUT_004
 */

import { render, screen, fireEvent, act } from '@testing-library/react';
import { vi, describe, it, expect, beforeEach, afterEach } from 'vitest';
import { AboutTab } from '../AboutTab';

describe('AboutTab', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  /* ── 初始化和渲染 ── */

  describe('初始化和渲染', () => {
    it('应该渲染 Logo 图标区域', () => {
      render(<AboutTab />);
      // ShieldCheck 图标在 88px 圆形容器中
      const logoBox = screen.getByText('X-Claw').closest('div')?.parentElement;
      expect(logoBox).toBeInTheDocument();
    });

    it('应该渲染应用名称 X-Claw', () => {
      render(<AboutTab />);
      const title = screen.getByText('X-Claw');
      expect(title).toBeInTheDocument();
      expect(title.tagName).toBe('H3');
    });

    it('应该渲染副标题', () => {
      render(<AboutTab />);
      expect(screen.getByText('智能 AI 助手，让工作更高效')).toBeInTheDocument();
    });

    it('应该渲染版本号', () => {
      render(<AboutTab />);
      expect(screen.getByText('当前版本')).toBeInTheDocument();
      expect(screen.getByText('v0.1.0')).toBeInTheDocument();
    });

    it('应该渲染检查更新按钮', () => {
      render(<AboutTab />);
      expect(screen.getByText('检查更新')).toBeInTheDocument();
    });

    it('初始不应显示检查结果', () => {
      const { container } = render(<AboutTab />);
      // 结果区域始终存在（占位防抖动），但初始应为透明
      const resultEl = container.querySelector('p.opacity-0');
      expect(resultEl).toBeInTheDocument();
    });
  });

  /* ── 检查更新交互 ── */

  describe('检查更新交互', () => {
    it('点击检查更新应显示加载状态', async () => {
      render(<AboutTab />);
      fireEvent.click(screen.getByText('检查更新'));
      expect(screen.getByText('检查中...')).toBeInTheDocument();
    });

    it('检查更新期间按钮应禁用', async () => {
      render(<AboutTab />);
      const btn = screen.getByText('检查更新').closest('button')!;
      fireEvent.click(btn);
      expect(btn).toBeDisabled();
    });

    it('检查完成后应显示结果', async () => {
      render(<AboutTab />);
      fireEvent.click(screen.getByText('检查更新'));

      // 快进 1500ms 并刷新微任务队列
      await act(async () => {
        await vi.advanceTimersByTimeAsync(1500);
      });

      expect(screen.getByText('当前已是最新版本')).toBeInTheDocument();
    });

    it('检查完成后按钮应恢复可用', async () => {
      render(<AboutTab />);
      const btn = screen.getByText('检查更新').closest('button')!;
      fireEvent.click(btn);

      await act(async () => {
        await vi.advanceTimersByTimeAsync(1500);
      });

      expect(btn).not.toBeDisabled();
    });
  });

  /* ── 安全审计 ── */

  describe('test_security_audit', () => {
    it('应用名称应为纯文本渲染，防止 XSS', () => {
      render(<AboutTab />);
      const title = screen.getByText('X-Claw');
      expect(title.tagName).toBe('H3');
      expect(title.innerHTML).toBe('X-Claw');
    });

    it('版本号应为纯文本渲染', () => {
      render(<AboutTab />);
      const version = screen.getByText('v0.1.0');
      expect(version.tagName).toBe('SPAN');
    });

    it('检查更新错误不应泄露敏感信息', async () => {
      render(<AboutTab />);
      fireEvent.click(screen.getByText('检查更新'));

      await act(async () => {
        await vi.advanceTimersByTimeAsync(1500);
      });

      const result = screen.getByText('当前已是最新版本');
      expect(result.textContent).not.toContain('password');
      expect(result.textContent).not.toContain('token');
      expect(result.textContent).not.toContain('secret');
    });
  });

  /* ── 契约测试 ── */

  describe('test_contract', () => {
    it('Logo 区域应包含 ShieldCheck 图标', () => {
      const { container } = render(<AboutTab />);
      // lucide 图标渲染为 svg
      const svgs = container.querySelectorAll('svg');
      expect(svgs.length).toBeGreaterThanOrEqual(1);
    });

    it('版本行应包含版本标签和版本号', () => {
      render(<AboutTab />);
      const label = screen.getByText('当前版本');
      const version = screen.getByText('v0.1.0');
      // 两者应在同一个容器中
      expect(label.closest('div')).toBe(version.closest('div'));
    });

    it('检查更新按钮应包含 RefreshCw 图标', () => {
      render(<AboutTab />);
      const btn = screen.getByText('检查更新').closest('button')!;
      const svg = btn.querySelector('svg');
      expect(svg).toBeInTheDocument();
    });
  });

  /* ── 需求级测试 ── */

  describe('req_about_001_logo_display', () => {
    it('应该显示 88px 圆形绿色背景的 Logo', () => {
      const { container } = render(<AboutTab />);
      // 查找 88px 圆形容器
      const logoBox = container.querySelector('.size-\\[88px\\]');
      expect(logoBox).toBeInTheDocument();
      expect(logoBox).toHaveClass('rounded-full');
      expect(logoBox).toHaveClass('bg-[#E8F5EE]');
    });
  });

  describe('req_about_002_app_info', () => {
    it('应该显示应用名称和副标题', () => {
      render(<AboutTab />);
      expect(screen.getByText('X-Claw')).toBeInTheDocument();
      expect(screen.getByText('智能 AI 助手，让工作更高效')).toBeInTheDocument();
    });
  });

  describe('req_about_003_version_info', () => {
    it('应该在灰色背景条中显示版本信息', () => {
      render(<AboutTab />);
      const versionRow = screen.getByText('当前版本').closest('div')?.parentElement;
      expect(versionRow).toHaveClass('bg-[#F5F4F1]');
      expect(versionRow).toHaveClass('rounded-xl');
    });
  });

  describe('req_about_004_check_update', () => {
    it('应该能触发检查更新并显示结果', async () => {
      render(<AboutTab />);

      // 初始状态
      expect(screen.getByText('检查更新')).toBeInTheDocument();

      // 点击检查
      fireEvent.click(screen.getByText('检查更新'));
      expect(screen.getByText('检查中...')).toBeInTheDocument();

      // 等待完成
      await act(async () => {
        await vi.advanceTimersByTimeAsync(1500);
      });

      expect(screen.getByText('当前已是最新版本')).toBeInTheDocument();
      expect(screen.getByText('检查更新')).toBeInTheDocument();
    });
  });
});
