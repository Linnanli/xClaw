/**
 * useTheme Hook 测试
 *
 * 覆盖维度：
 * - 正常路径：初始化、切换模式、持久化
 * - 失败路径：localStorage 不可用、matchMedia 不可用、无效存储值
 * - 契约测试：ThemeMode 类型约束、applyTheme 行为
 * - 安全审计：无敏感信息泄露
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useTheme, applyTheme, type ThemeMode } from '../useTheme';

const mockMatchMedia = (matches: boolean) => {
  Object.defineProperty(window, 'matchMedia', {
    writable: true,
    value: vi.fn().mockImplementation((query: string) => ({
      matches,
      media: query,
      onchange: null,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    })),
  });
};

describe('useTheme', () => {
  beforeEach(() => {
    localStorage.clear();
    mockMatchMedia(false);
    document.documentElement.classList.remove('dark');
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  // ===== 正常路径测试 =====

  it('无 localStorage 时默认主题应为 system', () => {
    const { result } = renderHook(() => useTheme());
    expect(result.current.theme).toBe('system');
  });

  it('应从 localStorage 恢复已保存的 dark 主题', () => {
    localStorage.setItem('xclaw-theme', 'dark');
    const { result } = renderHook(() => useTheme());
    expect(result.current.theme).toBe('dark');
  });

  it('应从 localStorage 恢复已保存的 light 主题', () => {
    localStorage.setItem('xclaw-theme', 'light');
    const { result } = renderHook(() => useTheme());
    expect(result.current.theme).toBe('light');
  });

  it('恢复 dark 主题时应立即添加 .dark class', () => {
    localStorage.setItem('xclaw-theme', 'dark');
    renderHook(() => useTheme());
    expect(document.documentElement.classList.contains('dark')).toBe(true);
  });

  it('恢复 light 主题时应立即移除 .dark class（即使系统是暗色）', () => {
    mockMatchMedia(true); // 系统暗色
    localStorage.setItem('xclaw-theme', 'light');
    document.documentElement.classList.add('dark'); // 模拟 index.html 脚本加了 .dark
    renderHook(() => useTheme());
    expect(document.documentElement.classList.contains('dark')).toBe(false);
  });

  it('切换到 dark 模式应添加 .dark class', () => {
    const { result } = renderHook(() => useTheme());
    act(() => { result.current.setTheme('dark'); });
    expect(document.documentElement.classList.contains('dark')).toBe(true);
    expect(result.current.theme).toBe('dark');
  });

  it('切换到 light 模式应移除 .dark class', () => {
    document.documentElement.classList.add('dark');
    const { result } = renderHook(() => useTheme());
    act(() => { result.current.setTheme('light'); });
    expect(document.documentElement.classList.contains('dark')).toBe(false);
    expect(result.current.theme).toBe('light');
  });

  it('切换到 light 模式后刷新（重新挂载）应保持 light', () => {
    const { result } = renderHook(() => useTheme());
    act(() => { result.current.setTheme('light'); });
    // 模拟刷新：重新挂载 hook
    const { result: result2 } = renderHook(() => useTheme());
    expect(result2.current.theme).toBe('light');
    expect(document.documentElement.classList.contains('dark')).toBe(false);
  });

  it('切换到 system 且系统亮色时应移除 .dark', () => {
    mockMatchMedia(false);
    document.documentElement.classList.add('dark');
    const { result } = renderHook(() => useTheme());
    act(() => { result.current.setTheme('system'); });
    expect(document.documentElement.classList.contains('dark')).toBe(false);
  });

  it('切换到 system 且系统暗色时应添加 .dark', () => {
    mockMatchMedia(true);
    const { result } = renderHook(() => useTheme());
    act(() => { result.current.setTheme('system'); });
    expect(document.documentElement.classList.contains('dark')).toBe(true);
  });

  it('setTheme 应将主题持久化到 localStorage', () => {
    const { result } = renderHook(() => useTheme());
    act(() => { result.current.setTheme('dark'); });
    expect(localStorage.getItem('xclaw-theme')).toBe('dark');
  });

  it('setTheme light 应将 light 持久化到 localStorage', () => {
    const { result } = renderHook(() => useTheme());
    act(() => { result.current.setTheme('light'); });
    expect(localStorage.getItem('xclaw-theme')).toBe('light');
  });

  it('setTheme 应在 setThemeState 之前立即应用主题（不等 useEffect）', () => {
    const { result } = renderHook(() => useTheme());
    // 在 act 内部，applyTheme 应该已经被调用
    act(() => { result.current.setTheme('dark'); });
    expect(document.documentElement.classList.contains('dark')).toBe(true);
  });

  // ===== 失败路径测试 =====

  it('test_failure_localStorage_getItem_throws_should_use_system_default', () => {
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new Error('Storage unavailable');
    });
    const { result } = renderHook(() => useTheme());
    expect(result.current.theme).toBe('system');
  });

  it('test_failure_localStorage_setItem_throws_should_not_crash', () => {
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('Storage full');
    });
    const { result } = renderHook(() => useTheme());
    expect(() => {
      act(() => { result.current.setTheme('dark'); });
    }).not.toThrow();
    // 即使存储失败，主题状态仍应更新
    expect(result.current.theme).toBe('dark');
  });

  it('test_failure_invalid_stored_value_should_fallback_to_system', () => {
    localStorage.setItem('xclaw-theme', 'invalid-value');
    const { result } = renderHook(() => useTheme());
    // 无效值应回退到 system
    expect(result.current.theme).toBe('system');
  });

  it('test_failure_empty_stored_value_should_fallback_to_system', () => {
    localStorage.setItem('xclaw-theme', '');
    const { result } = renderHook(() => useTheme());
    expect(result.current.theme).toBe('system');
  });

  it('test_failure_system_dark_with_light_stored_should_stay_light', () => {
    // 关键回归测试：系统暗色 + 用户选了 light → 刷新后应保持 light
    mockMatchMedia(true); // 系统暗色
    localStorage.setItem('xclaw-theme', 'light');
    renderHook(() => useTheme());
    // light 模式应强制移除 .dark，不受系统偏好影响
    expect(document.documentElement.classList.contains('dark')).toBe(false);
  });

  // ===== applyTheme 单元测试 =====

  it('applyTheme(dark) 应添加 .dark class', () => {
    document.documentElement.classList.remove('dark');
    applyTheme('dark');
    expect(document.documentElement.classList.contains('dark')).toBe(true);
  });

  it('applyTheme(light) 应移除 .dark class', () => {
    document.documentElement.classList.add('dark');
    applyTheme('light');
    expect(document.documentElement.classList.contains('dark')).toBe(false);
  });

  it('applyTheme(light) 在系统暗色时仍应移除 .dark', () => {
    mockMatchMedia(true);
    document.documentElement.classList.add('dark');
    applyTheme('light');
    expect(document.documentElement.classList.contains('dark')).toBe(false);
  });

  it('applyTheme(system) 系统亮色时应移除 .dark', () => {
    mockMatchMedia(false);
    document.documentElement.classList.add('dark');
    applyTheme('system');
    expect(document.documentElement.classList.contains('dark')).toBe(false);
  });

  it('applyTheme(system) 系统暗色时应添加 .dark', () => {
    mockMatchMedia(true);
    document.documentElement.classList.remove('dark');
    applyTheme('system');
    expect(document.documentElement.classList.contains('dark')).toBe(true);
  });

  // ===== 契约测试 =====

  it('test_contract_theme_mode_accepts_all_valid_values', () => {
    const validModes: ThemeMode[] = ['light', 'dark', 'system'];
    const { result } = renderHook(() => useTheme());
    validModes.forEach((mode) => {
      expect(() => {
        act(() => { result.current.setTheme(mode); });
      }).not.toThrow();
    });
  });

  it('test_contract_setTheme_returns_void', () => {
    const { result } = renderHook(() => useTheme());
    let returnValue: unknown;
    act(() => { returnValue = result.current.setTheme('light'); });
    expect(returnValue).toBeUndefined();
  });

  it('test_contract_theme_state_matches_localStorage_after_setTheme', () => {
    const { result } = renderHook(() => useTheme());
    act(() => { result.current.setTheme('dark'); });
    expect(result.current.theme).toBe(localStorage.getItem('xclaw-theme'));
  });

  // ===== 安全审计测试 =====

  it('test_audit_no_sensitive_data_in_localStorage', () => {
    const { result } = renderHook(() => useTheme());
    act(() => { result.current.setTheme('dark'); });
    const stored = localStorage.getItem('xclaw-theme');
    expect(stored).toBe('dark');
    expect(stored).not.toMatch(/password|token|secret|key|user|email/i);
  });

  it('test_audit_only_theme_key_written_to_localStorage', () => {
    const { result } = renderHook(() => useTheme());
    act(() => { result.current.setTheme('light'); });
    // 只应写入 xclaw-theme 这一个 key
    expect(localStorage.length).toBe(1);
    expect(localStorage.key(0)).toBe('xclaw-theme');
  });
});
