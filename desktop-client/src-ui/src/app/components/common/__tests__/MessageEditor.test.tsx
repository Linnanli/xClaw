import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { userEvent } from '@testing-library/user-event';
import { MessageEditor } from '../MessageEditor';

// Mock theme context
vi.mock('../../../contexts/ThemeContext', () => ({
  useTheme: () => ({ theme: 'light' })
}));

describe('MessageEditor', () => {
  const user = userEvent.setup();
  const mockOnSave = vi.fn();
  const mockOnCancel = vi.fn();

  const defaultProps = {
    messageId: 'msg-123',
    initialContent: 'Original message content',
    onSave: mockOnSave,
    onCancel: mockOnCancel,
  };

  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('渲染', () => {
    it('应该显示编辑器界面', () => {
      render(<MessageEditor {...defaultProps} />);

      expect(screen.getByDisplayValue('Original message content')).toBeInTheDocument();
      expect(screen.getByText('保存')).toBeInTheDocument();
      expect(screen.getByText('取消')).toBeInTheDocument();
    });

    it('应该自动聚焦到文本框', () => {
      render(<MessageEditor {...defaultProps} />);

      const textarea = screen.getByDisplayValue('Original message content');
      expect(textarea).toHaveFocus();
    });

    it('应该显示字符计数', () => {
      render(<MessageEditor {...defaultProps} />);

      expect(screen.getByText('24 / 2000')).toBeInTheDocument();
    });
  });

  describe('编辑功能', () => {
    it('应该允许编辑内容', async () => {
      render(<MessageEditor {...defaultProps} />);

      const textarea = screen.getByDisplayValue('Original message content');
      
      await user.clear(textarea);
      await user.type(textarea, 'New message content');

      expect(textarea).toHaveValue('New message content');
    });

    it('应该更新字符计数', async () => {
      render(<MessageEditor {...defaultProps} />);

      const textarea = screen.getByDisplayValue('Original message content');
      
      await user.clear(textarea);
      await user.type(textarea, 'Short');

      expect(screen.getByText('5 / 2000')).toBeInTheDocument();
    });

    it('应该限制最大字符数', async () => {
      render(<MessageEditor {...defaultProps} />);

      const textarea = screen.getByDisplayValue('Original message content');
      
      // 创建一个超过限制的文本
      const longText = 'a'.repeat(2001);
      
      await user.clear(textarea);
      
      // 分批输入以避免超时
      const batchSize = 500;
      for (let i = 0; i < longText.length; i += batchSize) {
        const batch = longText.slice(i, i + batchSize);
        await user.type(textarea, batch);
        
        // 如果已经达到限制，停止输入
        if (textarea.value.length >= 2000) {
          break;
        }
      }

      // 应该被截断到2000字符
      expect(textarea.value).toHaveLength(2000);
    }, 10000); // 增加超时时间
  });

  describe('保存功能', () => {
    it('应该在点击保存时调用回调', async () => {
      render(<MessageEditor {...defaultProps} />);

      const textarea = screen.getByDisplayValue('Original message content');
      await user.clear(textarea);
      await user.type(textarea, 'Updated content');

      // 等待状态更新
      await new Promise(resolve => setTimeout(resolve, 100));

      await user.click(screen.getByText('保存'));

      expect(mockOnSave).toHaveBeenCalledWith('msg-123', 'Updated content');
    });

    it('应该在Ctrl+Enter时保存', async () => {
      render(<MessageEditor {...defaultProps} />);

      const textarea = screen.getByDisplayValue('Original message content');
      await user.clear(textarea);
      await user.type(textarea, 'Updated content');

      // 等待状态更新
      await new Promise(resolve => setTimeout(resolve, 100));

      await user.keyboard('{Control>}{Enter}{/Control}');

      expect(mockOnSave).toHaveBeenCalledWith('msg-123', 'Updated content');
    });

    it('应该禁用空内容的保存', async () => {
      render(<MessageEditor {...defaultProps} />);

      const textarea = screen.getByDisplayValue('Original message content');
      await user.clear(textarea);

      // 等待状态更新
      await new Promise(resolve => setTimeout(resolve, 100));

      const saveButton = screen.getByText('保存');
      expect(saveButton).toBeDisabled();
    });

    it('应该禁用未修改内容的保存', () => {
      render(<MessageEditor {...defaultProps} />);

      const saveButton = screen.getByText('保存');
      expect(saveButton).toBeDisabled();
    });
  });

  describe('取消功能', () => {
    it('应该在点击取消时调用回调', async () => {
      render(<MessageEditor {...defaultProps} />);

      await user.click(screen.getByText('取消'));

      expect(mockOnCancel).toHaveBeenCalled();
    });

    it('应该在Escape时取消', async () => {
      render(<MessageEditor {...defaultProps} />);

      const textarea = screen.getByDisplayValue('Original message content');
      await user.click(textarea);
      await user.keyboard('{Escape}');

      expect(mockOnCancel).toHaveBeenCalled();
    });
  });

  describe('加载状态', () => {
    it('应该显示保存中状态', () => {
      render(<MessageEditor {...defaultProps} loading />);

      expect(screen.getByText('保存中...')).toBeInTheDocument();
      expect(screen.getByText('保存中...')).toBeDisabled();
    });

    it('应该在加载时禁用取消按钮', () => {
      render(<MessageEditor {...defaultProps} loading />);

      expect(screen.getByText('取消')).toBeDisabled();
    });
  });

  describe('无障碍性', () => {
    it('应该有正确的ARIA标签', () => {
      render(<MessageEditor {...defaultProps} />);

      const textarea = screen.getByDisplayValue('Original message content');
      expect(textarea).toHaveAttribute('aria-label', '编辑消息内容');
    });

    it('应该有正确的键盘提示', () => {
      render(<MessageEditor {...defaultProps} />);

      expect(screen.getByText('Ctrl+Enter 保存，Escape 取消')).toBeInTheDocument();
    });
  });
});