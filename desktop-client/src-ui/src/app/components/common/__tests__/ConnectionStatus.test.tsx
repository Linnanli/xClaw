import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { ConnectionStatus } from '../ConnectionStatus';
import { sseService } from '../../../services/sseService';

// Mock SSE service
vi.mock('../../../services/sseService', () => ({
  sseService: {
    getConnectionStatus: vi.fn(),
    onStatusChange: vi.fn(),
    offStatusChange: vi.fn(),
    isConnected: vi.fn()
  }
}));

// Mock theme context
vi.mock('../../../contexts/ThemeContext', () => ({
  useTheme: () => ({ theme: 'light' })
}));

describe('ConnectionStatus', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('应该显示已连接状态', () => {
    vi.mocked(sseService.getConnectionStatus).mockReturnValue('connected');
    vi.mocked(sseService.isConnected).mockReturnValue(true);

    render(<ConnectionStatus />);

    expect(screen.getByText('已连接')).toBeInTheDocument();
    expect(screen.getByTestId('connection-dot')).toHaveClass('bg-green-400');
  });

  it('应该显示连接中状态', () => {
    vi.mocked(sseService.getConnectionStatus).mockReturnValue('connecting');
    vi.mocked(sseService.isConnected).mockReturnValue(false);

    render(<ConnectionStatus />);

    expect(screen.getByText('连接中')).toBeInTheDocument();
    expect(screen.getByTestId('connection-dot')).toHaveClass('bg-yellow-400');
  });

  it('应该显示重连中状态', () => {
    vi.mocked(sseService.getConnectionStatus).mockReturnValue('reconnecting');
    vi.mocked(sseService.isConnected).mockReturnValue(false);

    render(<ConnectionStatus />);

    expect(screen.getByText('重连中')).toBeInTheDocument();
    expect(screen.getByTestId('connection-dot')).toHaveClass('bg-yellow-400');
  });

  it('应该显示已断开状态', () => {
    vi.mocked(sseService.getConnectionStatus).mockReturnValue('disconnected');
    vi.mocked(sseService.isConnected).mockReturnValue(false);

    render(<ConnectionStatus />);

    expect(screen.getByText('已断开')).toBeInTheDocument();
    expect(screen.getByTestId('connection-dot')).toHaveClass('bg-red-400');
  });

  it('应该显示连接失败状态', () => {
    vi.mocked(sseService.getConnectionStatus).mockReturnValue('failed');
    vi.mocked(sseService.isConnected).mockReturnValue(false);

    render(<ConnectionStatus />);

    expect(screen.getByText('连接失败')).toBeInTheDocument();
    expect(screen.getByTestId('connection-dot')).toHaveClass('bg-red-400');
  });

  it('应该注册和注销状态变化监听器', () => {
    const mockOnStatusChange = vi.mocked(sseService.onStatusChange);
    const mockOffStatusChange = vi.mocked(sseService.offStatusChange);

    const { unmount } = render(<ConnectionStatus />);

    expect(mockOnStatusChange).toHaveBeenCalledWith(expect.any(Function));

    unmount();

    expect(mockOffStatusChange).toHaveBeenCalledWith(expect.any(Function));
  });
});