import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { userEvent } from '@testing-library/user-event';
import { DeleteConfirmDialog } from '../DeleteConfirmDialog';

// Mock theme context
vi.mock('../../../contexts/ThemeContext', () => ({
  useTheme: () => ({ theme: 'light' })
}));

describe('DeleteConfirmDialog', () => {
  const user = userEvent.setup();
  const mockOnConfirm = vi.fn();
  const mockOnCancel = vi.fn();

  const defaultProps = {
    isOpen: true,
    title: '删除消息',
    message: '确定要删除这条消息吗？此操作无法撤销。',
    confirmText: '删除',
    cancelText: '取消',
    onConfirm: mockOnConfirm,
    onCancel: mockOnCancel,
  };

  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('渲染', () => {
    it('应该在打开时显示对话框', () => {
      render(<DeleteConfirmDialog {...defaultProps} />);

      expect(screen.getByText('删除消息')).toBeInTheDocument();
      expect(screen.getByText('确定要删除这条消息吗？此操作无法撤销。')).toBeInTheDocument();
      expect(screen.getByText('删除')).toBeInTheDocument();
      expect(screen.getByText('取消')).toBeInTheDocument();
    });

    it('应该在关闭时隐藏对话框', () => {
      render(<DeleteConfirmDialog {...defaultProps} isOpen={false} />);

      expect(screen.queryByText('删除消息')).not.toBeInTheDocument();
    });

    it('应该显示危险样式的确认按钮', () => {
      render(<DeleteConfirmDialog {...defaultProps} />);

      const confirmButton = screen.getByText('删除');
      expect(confirmButton).toHaveClass('bg-red-500');
    });
  });

  describe('交互', () => {
    it('应该在点击确认时调用回调', async () => {
      render(<DeleteConfirmDialog {...defaultProps} />);

      await user.click(screen.getByText('删除'));

      expect(mockOnConfirm).toHaveBeenCalled();
    });

    it('应该在点击取消时调用回调', async () => {
      render(<DeleteConfirmDialog {...defaultProps} />);

      await user.click(screen.getByText('取消'));

      expect(mockOnCancel).toHaveBeenCalled();
    });

    it('应该在点击遮罩时取消', async () => {
      render(<DeleteConfirmDialog {...defaultProps} />);

      const overlay = screen.getByTestId('dialog-overlay');
      await user.click(overlay);

      expect(mockOnCancel).toHaveBeenCalled();
    });

    it('应该在按Escape时取消', async () => {
      render(<DeleteConfirmDialog {...defaultProps} />);

      await user.keyboard('{Escape}');

      expect(mockOnCancel).toHaveBeenCalled();
    });
  });

  describe('加载状态', () => {
    it('应该显示加载状态', () => {
      render(<DeleteConfirmDialog {...defaultProps} loading />);

      expect(screen.getByText('删除中...')).toBeInTheDocument();
      expect(screen.getByText('删除中...')).toBeDisabled();
    });

    it('应该在加载时禁用取消按钮', () => {
      render(<DeleteConfirmDialog {...defaultProps} loading />);

      expect(screen.getByText('取消')).toBeDisabled();
    });
  });

  describe('键盘导航', () => {
    it('应该自动聚焦到取消按钮', () => {
      render(<DeleteConfirmDialog {...defaultProps} />);

      expect(screen.getByText('取消')).toHaveFocus();
    });

    it('应该支持Tab键导航', async () => {
      render(<DeleteConfirmDialog {...defaultProps} />);

      const cancelButton = screen.getByText('取消');
      const confirmButton = screen.getByText('删除');

      // 取消按钮应该已经有焦点
      expect(cancelButton).toHaveFocus();

      // Tab到确认按钮
      await user.tab();
      expect(confirmButton).toHaveFocus();

      // 再次Tab会跳到关闭按钮或跳出对话框
      // 在测试环境中，我们只验证前两个按钮的导航
      // 这已经足够验证基本的键盘导航功能
    });
  });

  describe('无障碍性', () => {
    it('应该有正确的ARIA属性', () => {
      render(<DeleteConfirmDialog {...defaultProps} />);

      const dialog = screen.getByRole('dialog');
      expect(dialog).toHaveAttribute('aria-labelledby');
      expect(dialog).toHaveAttribute('aria-describedby');
    });

    it('应该有正确的标题和描述ID', () => {
      render(<DeleteConfirmDialog {...defaultProps} />);

      const title = screen.getByText('删除消息');
      const description = screen.getByText('确定要删除这条消息吗？此操作无法撤销。');

      expect(title).toHaveAttribute('id');
      expect(description).toHaveAttribute('id');
    });
  });
});