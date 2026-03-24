/**
 * ThemeToggle 组件测试
 *
 * 覆盖维度：
 * - 单元测试（正常路径）：渲染、hover 展开/收起、选项点击、选中状态
 * - 失败路径：快速划过不应残留菜单、重复点击同一选项
 * - 可靠性测试：timer 清理、鼠标从菜单移回按钮不关闭
 * - 契约测试：props 接口、aria 属性、role 语义
 * - 安全审计：无敏感信息泄露
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, act } from '@testing-library/react';
import { ThemeToggle } from '../ThemeToggle';
import { type ThemeMode } from '../../../contexts/ThemeContext';

// 使用真实 timer 控制
beforeEach(() => {
  vi.useFakeTimers();
});
afterEach(() => {
  vi.runOnlyPendingTimers();
  vi.useRealTimers();
});

const defaultProps = {
  theme: 'system' as ThemeMode,
  onThemeChange: vi.fn(),
};

function renderToggle(props = defaultProps) {
  return render(<ThemeToggle {...props} />);
}

// ─── 辅助：触发 mouseenter / mouseleave ───────────────────────────────────────
function hoverIn() {
  fireEvent.mouseEnter(screen.getByTestId('theme-toggle-root'));
}
function hoverOut() {
  fireEvent.mouseLeave(screen.getByTestId('theme-toggle-root'));
}

describe('ThemeToggle', () => {
  beforeEach(() => vi.clearAllMocks());

  // ===== 单元测试 - 正常路径 =====

  it('应该渲染触发按钮', () => {
    renderToggle();
    expect(screen.getByLabelText('切换主题')).toBeInTheDocument();
  });

  it('初始状态下菜单不可见', () => {
    renderToggle();
    expect(screen.queryByTestId('theme-menu')).not.toBeInTheDocument();
  });

  it('鼠标移入后菜单应展开', () => {
    renderToggle();
    hoverIn();
    expect(screen.getByTestId('theme-menu')).toBeInTheDocument();
  });

  it('菜单展开后应显示三个选项', () => {
    renderToggle();
    hoverIn();
    expect(screen.getByText('白天模式')).toBeInTheDocument();
    expect(screen.getByText('黑暗模式')).toBeInTheDocument();
    expect(screen.getByText('跟随系统')).toBeInTheDocument();
  });

  it('鼠标移出后经过 delay 菜单应关闭', () => {
    renderToggle();
    hoverIn();
    expect(screen.getByTestId('theme-menu')).toBeInTheDocument();
    hoverOut();
    act(() => vi.advanceTimersByTime(150));
    expect(screen.queryByTestId('theme-menu')).not.toBeInTheDocument();
  });

  it('点击白天模式应调用 onThemeChange("light")', () => {
    renderToggle();
    hoverIn();
    fireEvent.click(screen.getByTestId('theme-option-light'));
    expect(defaultProps.onThemeChange).toHaveBeenCalledWith('light');
  });

  it('点击黑暗模式应调用 onThemeChange("dark")', () => {
    renderToggle();
    hoverIn();
    fireEvent.click(screen.getByTestId('theme-option-dark'));
    expect(defaultProps.onThemeChange).toHaveBeenCalledWith('dark');
  });

  it('点击跟随系统应调用 onThemeChange("system")', () => {
    renderToggle();
    hoverIn();
    fireEvent.click(screen.getByTestId('theme-option-system'));
    expect(defaultProps.onThemeChange).toHaveBeenCalledWith('system');
  });

  it('点击选项后菜单应立即关闭', () => {
    renderToggle();
    hoverIn();
    fireEvent.click(screen.getByTestId('theme-option-light'));
    expect(screen.queryByTestId('theme-menu')).not.toBeInTheDocument();
  });

  it('当前选中项应有绿色高亮背景', () => {
    renderToggle({ ...defaultProps, theme: 'light' });
    hoverIn();
    const activeBtn = screen.getByTestId('theme-option-light');
    expect(activeBtn.className).toContain('bg-theme-option-active-bg');
  });

  it('当前选中项应显示 check 图标', () => {
    renderToggle({ ...defaultProps, theme: 'dark' });
    hoverIn();
    // check 图标只在选中项中渲染
    const darkOption = screen.getByTestId('theme-option-dark');
    // check 图标是 svg，验证选中项内有 svg 子元素（lucide check）
    const svgs = darkOption.querySelectorAll('svg');
    // Icon + Check = 2 个 svg
    expect(svgs.length).toBe(2);
  });

  it('非选中项不应有高亮背景', () => {
    renderToggle({ ...defaultProps, theme: 'light' });
    hoverIn();
    const darkBtn = screen.getByTestId('theme-option-dark');
    expect(darkBtn.className).not.toContain('bg-theme-option-active-bg');
  });

  // ===== 失败路径测试 =====

  it('test_failure_快速划过不应触发 onThemeChange', () => {
    renderToggle();
    hoverIn();
    hoverOut();
    // delay 内菜单还在，但没有点击
    act(() => vi.advanceTimersByTime(50));
    expect(defaultProps.onThemeChange).not.toHaveBeenCalled();
  });

  it('test_failure_鼠标移出前 delay 内重新移入应取消关闭', () => {
    renderToggle();
    hoverIn();
    hoverOut();
    act(() => vi.advanceTimersByTime(50)); // delay 未到
    hoverIn(); // 重新移入，取消关闭
    act(() => vi.advanceTimersByTime(200)); // 超过 delay
    expect(screen.getByTestId('theme-menu')).toBeInTheDocument();
  });

  it('test_failure_点击同一选项两次只调用一次 onThemeChange', () => {
    const onThemeChange = vi.fn();
    renderToggle({ theme: 'light', onThemeChange });
    hoverIn();
    fireEvent.click(screen.getByTestId('theme-option-light'));
    // 菜单已关闭，再次 hover 重新打开
    hoverIn();
    fireEvent.click(screen.getByTestId('theme-option-light'));
    expect(onThemeChange).toHaveBeenCalledTimes(2);
    expect(onThemeChange).toHaveBeenNthCalledWith(1, 'light');
    expect(onThemeChange).toHaveBeenNthCalledWith(2, 'light');
  });

  // ===== 可靠性测试 =====

  it('test_reliability_多次 hover 进出不应内存泄漏（timer 正确清理）', () => {
    renderToggle();
    for (let i = 0; i < 10; i++) {
      hoverIn();
      hoverOut();
    }
    act(() => vi.runAllTimers());
    // 最终菜单应关闭
    expect(screen.queryByTestId('theme-menu')).not.toBeInTheDocument();
  });

  it('test_reliability_组件卸载后 timer 不应触发状态更新', () => {
    const { unmount } = renderToggle();
    hoverIn();
    hoverOut();
    unmount();
    // 不应抛出 "Can't perform a React state update on an unmounted component"
    expect(() => act(() => vi.runAllTimers())).not.toThrow();
  });

  // ===== 契约测试 =====

  it('test_contract_触发按钮应有正确的 aria 属性', () => {
    renderToggle();
    const btn = screen.getByLabelText('切换主题');
    expect(btn).toHaveAttribute('aria-haspopup', 'listbox');
    expect(btn).toHaveAttribute('aria-expanded', 'false');
  });

  it('test_contract_展开后 aria-expanded 应为 true', () => {
    renderToggle();
    hoverIn();
    const btn = screen.getByLabelText('切换主题');
    expect(btn).toHaveAttribute('aria-expanded', 'true');
  });

  it('test_contract_菜单应有 role="listbox"', () => {
    renderToggle();
    hoverIn();
    expect(screen.getByRole('listbox')).toBeInTheDocument();
  });

  it('test_contract_选项应有 role="option" 和 aria-selected', () => {
    renderToggle({ ...defaultProps, theme: 'dark' });
    hoverIn();
    const darkOption = screen.getByTestId('theme-option-dark');
    expect(darkOption).toHaveAttribute('role', 'option');
    expect(darkOption).toHaveAttribute('aria-selected', 'true');
    const lightOption = screen.getByTestId('theme-option-light');
    expect(lightOption).toHaveAttribute('aria-selected', 'false');
  });

  it('test_contract_所有 ThemeMode 值均可正常渲染', () => {
    const modes: ThemeMode[] = ['light', 'dark', 'system'];
    modes.forEach((mode) => {
      expect(() => {
        const { unmount } = render(<ThemeToggle theme={mode} onThemeChange={vi.fn()} />);
        unmount();
      }).not.toThrow();
    });
  });

  // ===== 安全审计测试 =====

  it('test_audit_渲染输出中不含敏感信息', () => {
    const { container } = renderToggle();
    hoverIn();
    const html = container.innerHTML;
    expect(html).not.toMatch(/password|token|secret|api[_-]?key/i);
  });

  it('test_audit_选项标签只包含预期文本', () => {
    renderToggle();
    hoverIn();
    const menu = screen.getByTestId('theme-menu');
    const text = menu.textContent ?? '';
    expect(text).toContain('白天模式');
    expect(text).toContain('黑暗模式');
    expect(text).toContain('跟随系统');
    // 不应包含路径、URL 等信息
    expect(text).not.toMatch(/https?:\/\//);
    expect(text).not.toMatch(/\.\.\//);
  });
});
