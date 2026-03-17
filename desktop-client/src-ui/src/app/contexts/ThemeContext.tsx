import { createContext, useContext, useState, useEffect, ReactNode } from 'react';
import { tracing } from '../utils/tracing';

export type ThemeMode = 'light' | 'dark' | 'system';
export type ResolvedTheme = 'light' | 'dark';

interface ThemeContextType {
  theme: ResolvedTheme;
  themeMode: ThemeMode;
  toggleTheme: () => void;
  setTheme: (mode: ThemeMode) => void;
}

const ThemeContext = createContext<ThemeContextType | undefined>(undefined);

const THEME_STORAGE_KEY = 'theme';

/**
 * 检测系统是否使用深色主题
 */
function getSystemTheme(): ResolvedTheme {
  if (typeof window === 'undefined') return 'light';
  
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

/**
 * 从localStorage获取保存的主题模式
 */
function getSavedThemeMode(): ThemeMode {
  if (typeof window === 'undefined') return 'system';
  
  try {
    const saved = localStorage.getItem(THEME_STORAGE_KEY);
    if (saved && ['light', 'dark', 'system'].includes(saved)) {
      return saved as ThemeMode;
    }
  } catch (error) {
    tracing.warn('Failed to read theme from localStorage', { error });
  }
  
  return 'system';
}

/**
 * 保存主题模式到localStorage
 */
function saveThemeMode(mode: ThemeMode): void {
  if (typeof window === 'undefined') return;
  
  try {
    localStorage.setItem(THEME_STORAGE_KEY, mode);
    tracing.debug('Theme saved to localStorage', { mode });
  } catch (error) {
    tracing.warn('Failed to save theme to localStorage', { error });
  }
}

/**
 * 解析主题模式为实际主题
 */
function resolveTheme(mode: ThemeMode): ResolvedTheme {
  if (mode === 'system') {
    return getSystemTheme();
  }
  return mode;
}

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [themeMode, setThemeModeState] = useState<ThemeMode>(() => getSavedThemeMode());
  const [theme, setThemeState] = useState<ResolvedTheme>(() => resolveTheme(getSavedThemeMode()));

  // 监听系统主题变化
  useEffect(() => {
    if (themeMode !== 'system') return;

    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
    
    const handleChange = (e: MediaQueryListEvent) => {
      const newTheme = e.matches ? 'dark' : 'light';
      setThemeState(newTheme);
      tracing.info('System theme changed', { theme: newTheme });
    };

    mediaQuery.addEventListener('change', handleChange);
    
    return () => {
      mediaQuery.removeEventListener('change', handleChange);
    };
  }, [themeMode]);

  // 更新主题模式
  const setTheme = (mode: ThemeMode) => {
    setThemeModeState(mode);
    const resolvedTheme = resolveTheme(mode);
    setThemeState(resolvedTheme);
    saveThemeMode(mode);
    
    tracing.info('Theme changed', { mode, resolvedTheme });
  };

  // 切换主题（在light和dark之间切换）
  const toggleTheme = () => {
    const newMode = theme === 'dark' ? 'light' : 'dark';
    setTheme(newMode);
  };

  // 应用主题到document
  useEffect(() => {
    const root = document.documentElement;
    
    // 移除之前的主题类
    root.classList.remove('light', 'dark');
    
    // 添加当前主题类
    root.classList.add(theme);
    
    // 设置CSS变量（可选，用于更复杂的主题系统）
    root.style.setProperty('--theme', theme);
    
    tracing.debug('Theme applied to document', { theme });
  }, [theme]);

  return (
    <ThemeContext.Provider value={{ theme, themeMode, toggleTheme, setTheme }}>
      {children}
    </ThemeContext.Provider>
  );
}

export function useTheme() {
  const context = useContext(ThemeContext);
  if (context === undefined) {
    throw new Error('useTheme must be used within a ThemeProvider');
  }
  return context;
}
