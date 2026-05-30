import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { userEvent } from '@testing-library/user-event';
import { MessageActions } from '../MessageActions';

// Mock theme context
vi.mock('../../../contexts/ThemeContext', () => ({
  useTheme: () => ({ theme: 'light' })
}));

describe('MessageActions', () => {
  const user = userEvent.setup();
  const mockOnEdit = vi.fn();
  const mockOnDelete = vi.fn();
  const mockOnCopy = vi.fn();

  const defaultProps = {
    messageId: 'msg-123',
    content: 'Test message content',
    canEdit: true,
    canDelete: true,
    onEdit: mockOnEdit,
    onDelete: mockOnDelete,
    onCopy: mockOnCopy,
  };

  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('渲染', () => {
    it('应该显示操作按钮', () => {
      render(<MessageActions {...defaultProps} />);

      expect(screen.getByLabelText('编辑消息')).toBeInTheDocument();
      expect(screen.getByLabelText('删除消息')).toBeInTheDocument();
      expect(screen.getByLabelText('复制消息')).toBeInTheDocument();
    });

    it('应该根据权限隐藏按钮', () => {
      render(
        <MessageActions 
          {...defaultProps} 
          canEdit={false} 
          canDelete={false} 
        />
      );

      expect(screen.queryByLabelText('编辑消息')).not.toBeInTheDocument();
      expect(screen.queryByLabelText('删除消息')).not.toBeInTheDocument();
      expect(screen.getByLabelText('复制消息')).toBeInTheDocument();
    });

    it('应该在group hover时显示操作按钮', () => {
      render(<MessageActions {...defaultProps} />);

      const container = screen.getByTestId('message-actions');
      
      // 初始状态下按钮应该是隐藏的
      expect(container).toHaveClass('opacity-0');
      expect(container).toHaveClass('group-hover:opacity-100');
    });
  });

  describe('操作', () => {
    it('应该调用编辑回调', async () => {
      render(<MessageActions {...defaultProps} />);

      await user.click(screen.getByLabelText('编辑消息'));

      expect(mockOnEdit).toHaveBeenCalledWith('msg-123');
    });

    it('应该调用删除回调', async () => {
      render(<MessageActions {...defaultProps} />);

      await user.click(screen.getByLabelText('删除消息'));

      expect(mockOnDelete).toHaveBeenCalledWith('msg-123');
    });

    it('应该调用复制回调', async () => {
      render(<MessageActions {...defaultProps} />);

      await user.click(screen.getByLabelText('复制消息'));

      expect(mockOnCopy).toHaveBeenCalledWith('Test message content');
    });
  });

  describe('键盘导航', () => {
    it('应该支持键盘操作', async () => {
      render(<MessageActions {...defaultProps} />);

      // 复制按钮是第一个，所以Tab会先聚焦到它
      const copyButton = screen.getByLabelText('复制消息');
      const editButton = screen.getByLabelText('编辑消息');
      
      // Tab导航到第一个按钮（复制）
      await user.tab();
      expect(copyButton).toHaveFocus();

      // Tab导航到第二个按钮（编辑）
      await user.tab();
      expect(editButton).toHaveFocus();

      // Enter键触发操作
      await user.keyboard('{Enter}');
      expect(mockOnEdit).toHaveBeenCalledWith('msg-123');
    });
  });

  describe('无障碍性', () => {
    it('应该有正确的ARIA标签', () => {
      render(<MessageActions {...defaultProps} />);

      expect(screen.getByLabelText('编辑消息')).toHaveAttribute('aria-label', '编辑消息');
      expect(screen.getByLabelText('删除消息')).toHaveAttribute('aria-label', '删除消息');
      expect(screen.getByLabelText('复制消息')).toHaveAttribute('aria-label', '复制消息');
    });

    it('应该有正确的role属性', () => {
      render(<MessageActions {...defaultProps} />);

      const container = screen.getByTestId('message-actions');
      expect(container).toHaveAttribute('role', 'toolbar');
    });
  });
});