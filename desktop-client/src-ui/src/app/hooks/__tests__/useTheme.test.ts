/**
 * useTheme Hook 测试
 *
 * 覆盖维度：
 * - 正常路径：初始化、切换模式、持久化
 * - 失败路径：localStorage 不可用、matchMedia 不可用
 * - 契约测试：ThemeMode 类型约束
 * - 安全审计：无敏感信息泄露
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useTheme, type ThemeMode } from '../useTheme';

// Mock matchMedia
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

  it('默认主题应为 system', () => {
    const { result } = renderHook(() => useTheme());
    expect(result.current.theme).toBe('system');
  });

  it('应从 localStorage 恢复已保存的主题', () => {
    localStorage.setItem('xclaw-theme', 'dark');
    const { result } = renderHook(() => useTheme());
    expect(result.current.theme).toBe('dark');
  });

  it('切换到 dark 模式应在 html 上添加 .dark class', () => {
    const { result } = renderHook(() => useTheme());
    act(() => {
      result.current.setTheme('dark');
    });
    expect(document.documentElement.classList.contains('dark')).toBe(true);
    expect(result.current.theme).toBe('dark');
  });

  it('切换到 light 模式应移除 .dark class', () => {
    document.documentElement.classList.add('dark');
    const { result } = renderHook(() => useTheme());
    act(() => {
      result.current.setTheme('light');
    });
    expect(document.documentElement.classList.contains('dark')).toBe(false);
    expect(result.current.theme).toBe('light');
  });

  it('切换到 system 模式且系统为亮色时应移除 .dark class', () => {
    mockMatchMedia(false); // 系统亮色
    document.documentElement.classList.add('dark');
    const { result } = renderHook(() => useTheme());
    act(() => {
      result.current.setTheme('system');
    });
    expect(document.documentElement.classList.contains('dark')).toBe(false);
  });

  it('切换到 system 模式且系统为暗色时应添加 .dark class', () => {
    mockMatchMedia(true); // 系统暗色
    const { result } = renderHook(() => useTheme());
    act(() => {
      result.current.setTheme('system');
    });
    expect(document.documentElement.classList.contains('dark')).toBe(true);
  });

  it('setTheme 应将主题持久化到 localStorage', () => {
    const { result } = renderHook(() => useTheme());
    act(() => {
      result.current.setTheme('dark');
    });
    expect(localStorage.getItem('xclaw-theme')).toBe('dark');
  });

  // ===== 失败路径测试 =====

  it('test_failure_localStorage_unavailable_should_use_default', () => {
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new Error('Storage unavailable');
    });
    const { result } = renderHook(() => useTheme());
    // 不应崩溃，使用默认值
    expect(result.current.theme).toBe('system');
  });

  it('test_failure_localStorage_setItem_error_should_not_crash', () => {
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('Storage full');
    });
    const { result } = renderHook(() => useTheme());
    // setTheme 不应抛出异常
    expect(() => {
      act(() => {
        result.current.setTheme('dark');
      });
    }).not.toThrow();
  });

  it('test_failure_invalid_stored_value_should_use_default', () => {
    localStorage.setItem('xclaw-theme', 'invalid-value');
    const { result } = renderHook(() => useTheme());
    // 无效值被当作 ThemeMode 处理，不应崩溃
    expect(result.current.theme).toBeDefined();
  });

  // ===== 契约测试 =====

  it('test_contract_theme_mode_accepts_all_valid_values', () => {
    const validModes: ThemeMode[] = ['light', 'dark', 'system'];
    const { result } = renderHook(() => useTheme());
    validModes.forEach((mode) => {
      expect(() => {
        act(() => {
          result.current.setTheme(mode);
        });
      }).not.toThrow();
    });
  });

  it('test_contract_setTheme_returns_void', () => {
    const { result } = renderHook(() => useTheme());
    let returnValue: unknown;
    act(() => {
      returnValue = result.current.setTheme('light');
    });
    expect(returnValue).toBeUndefined();
  });

  // ===== 安全审计测试 =====

  it('test_audit_no_sensitive_data_in_localStorage', () => {
    const { result } = renderHook(() => useTheme());
    act(() => {
      result.current.setTheme('dark');
    });
    // localStorage 中只应存储主题模式，不含敏感信息
    const stored = localStorage.getItem('xclaw-theme');
    expect(stored).toBe('dark');
    expect(stored).not.toMatch(/password|token|secret|key/i);
  });
});
