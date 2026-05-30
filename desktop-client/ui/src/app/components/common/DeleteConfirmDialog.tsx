import { useEffect, useRef } from 'react';
import { AlertTriangle, X } from 'lucide-react';
import { useTheme } from '../../contexts/ThemeContext';

export interface DeleteConfirmDialogProps {
  isOpen: boolean;
  title: string;
  message: string;
  confirmText?: string;
  cancelText?: string;
  loading?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

/**
 * 删除确认对话框组件
 * 
 * 提供危险操作的确认界面
 */
export function DeleteConfirmDialog({
  isOpen,
  title,
  message,
  confirmText = '删除',
  cancelText = '取消',
  loading = false,
  onConfirm,
  onCancel,
}: DeleteConfirmDialogProps) {
  const { theme } = useTheme();
  const cancelButtonRef = useRef<HTMLButtonElement>(null);
  const titleId = `dialog-title-${Math.random().toString(36).substr(2, 9)}`;
  const descriptionId = `dialog-description-${Math.random().toString(36).substr(2, 9)}`;

  // 处理键盘事件
  useEffect(() => {
    if (!isOpen) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        onCancel();
      }
    };

    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, onCancel]);

  // 自动聚焦到取消按钮
  useEffect(() => {
    if (isOpen && cancelButtonRef.current) {
      cancelButtonRef.current.focus();
    }
  }, [isOpen]);

  // 阻止背景滚动
  useEffect(() => {
    if (isOpen) {
      document.body.style.overflow = 'hidden';
    } else {
      document.body.style.overflow = '';
    }

    return () => {
      document.body.style.overflow = '';
    };
  }, [isOpen]);

  if (!isOpen) return null;

  const handleOverlayClick = (e: React.MouseEvent) => {
    if (e.target === e.currentTarget) {
      onCancel();
    }
  };

  return (
    <div
      data-testid="dialog-overlay"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black bg-opacity-50"
      onClick={handleOverlayClick}
    >
      <div
        role="dialog"
        aria-labelledby={titleId}
        aria-describedby={descriptionId}
        aria-modal="true"
        className={`relative w-full max-w-md mx-4 rounded-lg shadow-xl ${
          theme === 'dark'
            ? 'bg-gray-800 border border-gray-700'
            : 'bg-white border border-gray-200'
        }`}
        onClick={(e) => e.stopPropagation()}
      >
        {/* 关闭按钮 */}
        <button
          onClick={onCancel}
          disabled={loading}
          className={`absolute top-4 right-4 flex size-7 items-center justify-center rounded-md bg-secondary text-muted-foreground transition-colors hover:bg-accent hover:text-foreground disabled:pointer-events-none disabled:opacity-50`}
          aria-label="关闭对话框"
        >
          <X size={16} />
        </button>

        {/* 内容区域 */}
        <div className="p-6">
          {/* 图标和标题 */}
          <div className="flex items-center gap-3 mb-4">
            <div className="flex-shrink-0 w-10 h-10 rounded-full bg-red-100 dark:bg-red-900/30 flex items-center justify-center">
              <AlertTriangle className="w-5 h-5 text-red-600 dark:text-red-400" />
            </div>
            <h3
              id={titleId}
              className={`text-lg font-semibold ${
                theme === 'dark' ? 'text-white' : 'text-gray-900'
              }`}
            >
              {title}
            </h3>
          </div>

          {/* 消息内容 */}
          <p
            id={descriptionId}
            className={`text-sm mb-6 ${
              theme === 'dark' ? 'text-gray-300' : 'text-gray-600'
            }`}
          >
            {message}
          </p>

          {/* 操作按钮 */}
          <div className="flex items-center gap-3 justify-end">
            <button
              ref={cancelButtonRef}
              onClick={onCancel}
              disabled={loading}
              className={`px-4 py-2 text-sm font-medium rounded-md transition-colors ${
                loading
                  ? theme === 'dark'
                    ? 'bg-gray-700 text-gray-500 cursor-not-allowed'
                    : 'bg-gray-200 text-gray-400 cursor-not-allowed'
                  : theme === 'dark'
                    ? 'bg-gray-700 text-gray-300 hover:bg-gray-600 focus:ring-2 focus:ring-gray-500'
                    : 'bg-gray-200 text-gray-700 hover:bg-gray-300 focus:ring-2 focus:ring-gray-400'
              }`}
            >
              {cancelText}
            </button>

            <button
              onClick={onConfirm}
              disabled={loading}
              className={`px-4 py-2 text-sm font-medium rounded-md transition-colors ${
                loading
                  ? 'bg-red-400 text-white cursor-not-allowed opacity-50'
                  : 'bg-red-500 text-white hover:bg-red-600 focus:ring-2 focus:ring-red-400'
              }`}
            >
              {loading ? `${confirmText}中...` : confirmText}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}