/**
 * ThemeToggle - 主题切换按钮 + 悬停下拉菜单
 *
 * 设计稿（X-Claw DLP Warning - Theme Toggle）：
 * - 右上角 32x32 sun 图标按钮
 * - 鼠标悬停时在按钮右下方展开 200px 宽的选项菜单
 * - 三个选项：白天模式 / 黑暗模式 / 跟随系统
 * - 当前选中项：绿色高亮背景 + check 图标
 */

import { useRef, useState, useCallback, useEffect } from 'react';
import { Sun, Moon, Monitor, Check } from 'lucide-react';
import { cn } from '../ui/utils';
import { type ThemeMode } from '../../contexts/ThemeContext';

interface ThemeOption {
  value: ThemeMode;
  label: string;
  Icon: React.ElementType;
}

const THEME_OPTIONS: ThemeOption[] = [
  { value: 'light', label: '白天模式', Icon: Sun },
  { value: 'dark', label: '黑暗模式', Icon: Moon },
  { value: 'system', label: '跟随系统', Icon: Monitor },
];

interface ThemeToggleProps {
  theme: ThemeMode;
  onThemeChange: (mode: ThemeMode) => void;
}

export function ThemeToggle({ theme, onThemeChange }: ThemeToggleProps) {
  const [open, setOpen] = useState(false);
  // 用 ref 存 timer，避免鼠标快速划过时菜单闪烁
  const closeTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const cancelClose = useCallback(() => {
    if (closeTimer.current) {
      clearTimeout(closeTimer.current);
      closeTimer.current = null;
    }
  }, []);

  // 组件卸载时清理 timer，防止 setState on unmounted component
  useEffect(() => () => cancelClose(), [cancelClose]);

  const scheduleClose = useCallback(() => {
    closeTimer.current = setTimeout(() => setOpen(false), 120);
  }, []);

  const handleMouseEnter = useCallback(() => {
    cancelClose();
    setOpen(true);
  }, [cancelClose]);

  const handleMouseLeave = useCallback(() => {
    scheduleClose();
  }, [scheduleClose]);

  const handleSelect = useCallback(
    (mode: ThemeMode) => {
      onThemeChange(mode);
      setOpen(false);
    },
    [onThemeChange],
  );

  return (
    <div
      className="relative"
      onMouseEnter={handleMouseEnter}
      onMouseLeave={handleMouseLeave}
      data-testid="theme-toggle-root"
    >
      {/* 触发按钮 */}
      <button
        className={cn(
          'flex size-8 items-center justify-center rounded-lg border border-border bg-secondary transition-colors',
          open && 'bg-accent',
        )}
        aria-label="切换主题"
        aria-haspopup="listbox"
        aria-expanded={open}
        type="button"
      >
        <Sun className="size-4 text-text-secondary" />
      </button>

      {/* 下拉菜单 */}
      {open && (
        <div
          role="listbox"
          aria-label="主题选项"
          className={cn(
            'absolute right-0 top-full z-50 mt-2 w-[200px] overflow-hidden rounded-[10px] border border-border bg-popover py-1.5',
            'shadow-[0_4px_16px_var(--shadow-color,rgba(0,0,0,0.10))]',
          )}
          data-testid="theme-menu"
        >
          {THEME_OPTIONS.map(({ value, label, Icon }) => {
            const isActive = theme === value;
            return (
              <button
                key={value}
                role="option"
                aria-selected={isActive}
                type="button"
                onClick={() => handleSelect(value)}
                className={cn(
                  'flex w-full items-center gap-3 rounded-md px-3 py-2.5 text-sm font-medium transition-colors',
                  isActive
                    ? 'bg-theme-option-active-bg text-primary'
                    : 'text-foreground hover:bg-secondary',
                )}
                data-testid={`theme-option-${value}`}
              >
                <Icon
                  className={cn('size-4 shrink-0', isActive ? 'text-primary' : 'text-text-secondary')}
                />
                <span className="flex-1 text-left">{label}</span>
                {isActive && <Check className="size-3.5 shrink-0 text-primary" />}
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
