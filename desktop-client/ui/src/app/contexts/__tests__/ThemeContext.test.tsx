import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { render, screen, act } from '@testing-library/react';
import { userEvent } from '@testing-library/user-event';
import { ThemeProvider, useTheme } from '../ThemeContext';

// Mock localStorage
const localStorageMock = {
  getItem: vi.fn(),
  setItem: vi.fn(),
  removeItem: vi.fn(),
  clear: vi.fn(),
};

Object.defineProperty(window, 'localStorage', {
  value: localStorageMock
});

// Mock matchMedia for system theme detection
const matchMediaMock = vi.fn();
Object.defineProperty(window, 'matchMedia', {
  value: matchMediaMock
});

// Test component that uses theme
function TestComponent() {
  const { theme, toggleTheme, setTheme } = useTheme();
  
  return (
    <div>
      <div data-testid="current-theme">{theme}</div>
      <button data-testid="toggle-theme" onClick={toggleTheme}>
        Toggle Theme
      </button>
      <button data-testid="set-light" onClick={() => setTheme('light')}>
        Set Light
      </button>
      <button data-testid="set-dark" onClick={() => setTheme('dark')}>
        Set Dark
      </button>
      <button data-testid="set-system" onClick={() => setTheme('system')}>
        Set System
      </button>
    </div>
  );
}

describe('ThemeContext', () => {
  const user = userEvent.setup();

  beforeEach(() => {
    vi.clearAllMocks();
    
    // Default matchMedia mock (light theme)
    matchMediaMock.mockReturnValue({
      matches: false,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    });
  });

  afterEach(() => {
    localStorageMock.clear();
  });

  describe('初始化', () => {
    it('应该使用默认主题（系统主题）', () => {
      localStorageMock.getItem.mockReturnValue(null);
      
      render(
        <ThemeProvider>
          <TestComponent />
        </ThemeProvider>
      );

      expect(screen.getByTestId('current-theme')).toHaveTextContent('light');
    });

    it('应该从localStorage加载保存的主题', () => {
      localStorageMock.getItem.mockReturnValue('dark');
      
      render(
        <ThemeProvider>
          <TestComponent />
        </ThemeProvider>
      );

      expect(screen.getByTestId('current-theme')).toHaveTextContent('dark');
    });

    it('应该检测系统深色主题', () => {
      localStorageMock.getItem.mockReturnValue('system');
      matchMediaMock.mockReturnValue({
        matches: true, // 系统使用深色主题
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
      });
      
      render(
        <ThemeProvider>
          <TestComponent />
        </ThemeProvider>
      );

      expect(screen.getByTestId('current-theme')).toHaveTextContent('dark');
    });
  });

  describe('主题切换', () => {
    it('应该能够切换主题', async () => {
      localStorageMock.getItem.mockReturnValue('light');
      
      render(
        <ThemeProvider>
          <TestComponent />
        </ThemeProvider>
      );

      expect(screen.getByTestId('current-theme')).toHaveTextContent('light');

      await user.click(screen.getByTestId('toggle-theme'));

      expect(screen.getByTestId('current-theme')).toHaveTextContent('dark');
    });

    it('应该能够设置特定主题', async () => {
      render(
        <ThemeProvider>
          <TestComponent />
        </ThemeProvider>
      );

      await user.click(screen.getByTestId('set-dark'));
      expect(screen.getByTestId('current-theme')).toHaveTextContent('dark');

      await user.click(screen.getByTestId('set-light'));
      expect(screen.getByTestId('current-theme')).toHaveTextContent('light');
    });

    it('应该能够设置系统主题', async () => {
      matchMediaMock.mockReturnValue({
        matches: true, // 系统使用深色主题
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
      });

      render(
        <ThemeProvider>
          <TestComponent />
        </ThemeProvider>
      );

      await user.click(screen.getByTestId('set-system'));
      expect(screen.getByTestId('current-theme')).toHaveTextContent('dark');
    });
  });

  describe('持久化', () => {
    it('应该保存主题到localStorage', async () => {
      render(
        <ThemeProvider>
          <TestComponent />
        </ThemeProvider>
      );

      await user.click(screen.getByTestId('set-dark'));

      expect(localStorageMock.setItem).toHaveBeenCalledWith('xclaw-theme', 'dark');
    });

    it('应该保存系统主题偏好', async () => {
      render(
        <ThemeProvider>
          <TestComponent />
        </ThemeProvider>
      );

      await user.click(screen.getByTestId('set-system'));

      expect(localStorageMock.setItem).toHaveBeenCalledWith('xclaw-theme', 'system');
    });
  });

  describe('系统主题监听', () => {
    it('应该监听系统主题变化', () => {
      const addEventListenerSpy = vi.fn();
      matchMediaMock.mockReturnValue({
        matches: false,
        addEventListener: addEventListenerSpy,
        removeEventListener: vi.fn(),
      });

      localStorageMock.getItem.mockReturnValue('system');

      render(
        <ThemeProvider>
          <TestComponent />
        </ThemeProvider>
      );

      expect(addEventListenerSpy).toHaveBeenCalledWith('change', expect.any(Function));
    });

    it('应该在组件卸载时清理监听器', () => {
      const removeEventListenerSpy = vi.fn();
      matchMediaMock.mockReturnValue({
        matches: false,
        addEventListener: vi.fn(),
        removeEventListener: removeEventListenerSpy,
      });

      localStorageMock.getItem.mockReturnValue('system');

      const { unmount } = render(
        <ThemeProvider>
          <TestComponent />
        </ThemeProvider>
      );

      unmount();

      expect(removeEventListenerSpy).toHaveBeenCalledWith('change', expect.any(Function));
    });
  });

  describe('错误处理', () => {
    it('应该在ThemeProvider外使用时抛出错误', () => {
      // 捕获console.error以避免测试输出中的错误信息
      const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

      expect(() => {
        render(<TestComponent />);
      }).toThrow('useTheme must be used within a ThemeProvider');

      consoleSpy.mockRestore();
    });
  });
});