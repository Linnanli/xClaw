import { useState, useEffect, useRef } from 'react';
import { Save, X } from 'lucide-react';
import { useTheme } from '../../contexts/ThemeContext';

export interface MessageEditorProps {
  messageId: string;
  initialContent: string;
  loading?: boolean;
  onSave: (messageId: string, content: string) => void;
  onCancel: () => void;
}

const MAX_CONTENT_LENGTH = 2000;

/**
 * 消息编辑器组件
 * 
 * 提供内联消息编辑功能
 */
export function MessageEditor({
  messageId,
  initialContent,
  loading = false,
  onSave,
  onCancel,
}: MessageEditorProps) {
  const { theme } = useTheme();
  const [content, setContent] = useState(initialContent);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // 自动聚焦和选择文本
  useEffect(() => {
    if (textareaRef.current) {
      textareaRef.current.focus();
      textareaRef.current.select();
    }
  }, []);

  // 自动调整文本框高度
  useEffect(() => {
    if (textareaRef.current) {
      textareaRef.current.style.height = 'auto';
      textareaRef.current.style.height = `${textareaRef.current.scrollHeight}px`;
    }
  }, [content]);

  const handleContentChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const newContent = e.target.value;
    
    // 限制最大长度
    if (newContent.length <= MAX_CONTENT_LENGTH) {
      setContent(newContent);
    }
  };

  const handleSave = () => {
    if (canSave) {
      onSave(messageId, content.trim());
    }
  };

  const handleCancel = () => {
    onCancel();
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      handleSave();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      handleCancel();
    }
  };

  const canSave = content.trim() !== '' && content.trim() !== initialContent.trim() && !loading;
  const characterCount = content.length;

  return (
    <div className={`border rounded-lg p-3 ${
      theme === 'dark'
        ? 'bg-gray-800 border-gray-600'
        : 'bg-white border-gray-300'
    }`}>
      {/* 文本编辑区 */}
      <textarea
        ref={textareaRef}
        value={content}
        onChange={handleContentChange}
        onKeyDown={handleKeyDown}
        aria-label="编辑消息内容"
        placeholder="输入消息内容..."
        className={`w-full min-h-[80px] resize-none border-none outline-none ${
          theme === 'dark'
            ? 'bg-transparent text-white placeholder-gray-400'
            : 'bg-transparent text-gray-900 placeholder-gray-500'
        }`}
        disabled={loading}
      />

      {/* 底部工具栏 */}
      <div className="flex items-center justify-between mt-3 pt-3 border-t border-gray-300 dark:border-gray-600">
        {/* 字符计数和提示 */}
        <div className="flex items-center gap-4">
          <span className={`text-xs ${
            characterCount > MAX_CONTENT_LENGTH * 0.9
              ? 'text-red-500'
              : theme === 'dark'
                ? 'text-gray-400'
                : 'text-gray-500'
          }`}>
            {characterCount} / {MAX_CONTENT_LENGTH}
          </span>
          
          <span className={`text-xs ${
            theme === 'dark' ? 'text-gray-400' : 'text-gray-500'
          }`}>
            Ctrl+Enter 保存，Escape 取消
          </span>
        </div>

        {/* 操作按钮 */}
        <div className="flex items-center gap-2">
          <button
            onClick={handleCancel}
            disabled={loading}
            className={`px-3 py-1.5 text-sm rounded-md transition-colors ${
              loading
                ? theme === 'dark'
                  ? 'bg-gray-700 text-gray-500 cursor-not-allowed'
                  : 'bg-gray-200 text-gray-400 cursor-not-allowed'
                : theme === 'dark'
                  ? 'bg-gray-700 text-gray-300 hover:bg-gray-600'
                  : 'bg-gray-200 text-gray-700 hover:bg-gray-300'
            }`}
          >
            <X size={14} className="inline mr-1" />
            取消
          </button>

          <button
            onClick={handleSave}
            disabled={!canSave}
            className={`px-3 py-1.5 text-sm rounded-md transition-colors flex items-center gap-1 ${
              !canSave
                ? theme === 'dark'
                  ? 'bg-gray-700 text-gray-500 cursor-not-allowed'
                  : 'bg-gray-200 text-gray-400 cursor-not-allowed'
                : theme === 'dark'
                  ? 'bg-blue-600 text-white hover:bg-blue-700'
                  : 'bg-blue-500 text-white hover:bg-blue-600'
            }`}
          >
            <Save size={14} />
            {loading ? '保存中...' : '保存'}
          </button>
        </div>
      </div>
    </div>
  );
}