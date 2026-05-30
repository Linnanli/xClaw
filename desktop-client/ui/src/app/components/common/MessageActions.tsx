import { Edit2, Trash2, Copy } from 'lucide-react';
import { useTheme } from '../../contexts/ThemeContext';

export interface MessageActionsProps {
  messageId: string;
  content: string;
  canEdit?: boolean;
  canDelete?: boolean;
  onEdit: (messageId: string) => void;
  onDelete: (messageId: string) => void;
  onCopy: (content: string) => void;
}

/**
 * 消息操作组件
 * 
 * 提供消息的编辑、删除、复制等操作
 */
export function MessageActions({
  messageId,
  content,
  canEdit = true,
  canDelete = true,
  onEdit,
  onDelete,
  onCopy,
}: MessageActionsProps) {
  const { theme } = useTheme();

  const handleEdit = () => {
    onEdit(messageId);
  };

  const handleDelete = () => {
    onDelete(messageId);
  };

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(content);
      onCopy(content);
    } catch (error) {
      // 如果clipboard API不可用，使用fallback方法
      const textArea = document.createElement('textarea');
      textArea.value = content;
      document.body.appendChild(textArea);
      textArea.select();
      document.execCommand('copy');
      document.body.removeChild(textArea);
      onCopy(content);
    }
  };

  return (
    <div
      data-testid="message-actions"
      role="toolbar"
      aria-label="消息操作"
      className={`flex items-center gap-1 transition-opacity duration-200 opacity-0 group-hover:opacity-100 ${
        theme === 'dark' ? 'bg-gray-800' : 'bg-white'
      } rounded-md shadow-lg border ${
        theme === 'dark' ? 'border-gray-600' : 'border-gray-200'
      } p-1`}
    >
      {/* 复制按钮 */}
      <button
        onClick={handleCopy}
        aria-label="复制消息"
        className={`p-1.5 rounded-md transition-colors ${
          theme === 'dark'
            ? 'hover:bg-gray-600 text-gray-400 hover:text-gray-200'
            : 'hover:bg-gray-200 text-gray-500 hover:text-gray-700'
        }`}
        title="复制消息"
      >
        <Copy size={14} />
      </button>

      {/* 编辑按钮 */}
      {canEdit && (
        <button
          onClick={handleEdit}
          aria-label="编辑消息"
          className={`p-1.5 rounded-md transition-colors ${
            theme === 'dark'
              ? 'hover:bg-gray-600 text-gray-400 hover:text-gray-200'
              : 'hover:bg-gray-200 text-gray-500 hover:text-gray-700'
          }`}
          title="编辑消息"
        >
          <Edit2 size={14} />
        </button>
      )}

      {/* 删除按钮 */}
      {canDelete && (
        <button
          onClick={handleDelete}
          aria-label="删除消息"
          className={`p-1.5 rounded-md transition-colors ${
            theme === 'dark'
              ? 'hover:bg-red-600 text-gray-400 hover:text-red-200'
              : 'hover:bg-red-100 text-gray-500 hover:text-red-600'
          }`}
          title="删除消息"
        >
          <Trash2 size={14} />
        </button>
      )}
    </div>
  );
}