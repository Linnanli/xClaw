/**
 * useTheme - 主题切换 Hook
 *
 * 支持三种模式：light（白天）、dark（黑暗）、system（跟随系统）
 * 持久化到 localStorage，并在 <html> 上切换 .dark class。
 *
 * 设计原则：
 * - light 模式：强制移除 .dark，不受系统偏好影响
 * - dark 模式：强制添加 .dark
 * - system 模式：跟随 prefers-color-scheme
 */

import { useState, useEffect, useCallback } from 'react';

export type ThemeMode = 'light' | 'dark' | 'system';

const STORAGE_KEY = 'xclaw-theme';
const VALID_MODES: ThemeMode[] = ['light', 'dark', 'system'];

function getSystemDark(): boolean {
  try {
    return window.matchMedia('(prefers-color-scheme: dark)').matches;
  } catch {
    return false;
  }
}

export function applyTheme(mode: ThemeMode): void {
  const root = document.documentElement;
  if (mode === 'dark') {
    root.classList.add('dark');
  } else if (mode === 'light') {
    // 显式移除，不依赖 toggle 的布尔值，防止竞态
    root.classList.remove('dark');
  } else {
    // system
    if (getSystemDark()) {
      root.classList.add('dark');
    } else {
      root.classList.remove('dark');
    }
  }
}

function readStoredTheme(): ThemeMode {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored && (VALID_MODES as string[]).includes(stored)) {
      return stored as ThemeMode;
    }
  } catch {
    // ignore
  }
  return 'system';
}

export function useTheme() {
  const [theme, setThemeState] = useState<ThemeMode>(() => {
    const mode = readStoredTheme();
    // 同步应用，防止 useEffect 延迟导致的闪烁
    applyTheme(mode);
    return mode;
  });

  // theme 变化时应用（处理 system 模式下的初始化）
  useEffect(() => {
    applyTheme(theme);
  }, [theme]);

  // 监听系统主题变化（仅 system 模式下生效）
  useEffect(() => {
    if (theme !== 'system') return;
    const mq = window.matchMedia('(prefers-color-scheme: dark)');
    const handler = () => applyTheme('system');
    mq.addEventListener('change', handler);
    return () => mq.removeEventListener('change', handler);
  }, [theme]);

  const setTheme = useCallback((mode: ThemeMode) => {
    try {
      localStorage.setItem(STORAGE_KEY, mode);
    } catch {
      // ignore storage errors
    }
    applyTheme(mode);
    setThemeState(mode);
  }, []);

  return { theme, setTheme };
}
