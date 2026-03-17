import { describe, it, expect, beforeEach, vi } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useSSEConnection } from '../useSSEConnection';
import { sseService } from '../../services/sseService';

// Mock SSE service
vi.mock('../../services/sseService', () => ({
  sseService: {
    connect: vi.fn(),
    disconnect: vi.fn(),
    isConnected: vi.fn(),
    getConnectionStatus: vi.fn(),
    on: vi.fn(),
    off: vi.fn(),
    onStatusChange: vi.fn(),
    offStatusChange: vi.fn()
  }
}));

describe('useSSEConnection', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('应该初始化连接', async () => {
    const mockConnect = vi.mocked(sseService.connect);
    mockConnect.mockResolvedValue();

    const { result } = renderHook(() => useSSEConnection());

    await act(async () => {
      await result.current.connect('test-token', 'http://localhost:3000');
    });

    expect(mockConnect).toHaveBeenCalledWith('test-token', 'http://localhost:3000');
  });

  it('应该断开连接', () => {
    const mockDisconnect = vi.mocked(sseService.disconnect);

    const { result } = renderHook(() => useSSEConnection());

    act(() => {
      result.current.disconnect();
    });

    expect(mockDisconnect).toHaveBeenCalled();
  });

  it('应该返回连接状态', () => {
    vi.mocked(sseService.isConnected).mockReturnValue(true);
    vi.mocked(sseService.getConnectionStatus).mockReturnValue('connected');

    const { result } = renderHook(() => useSSEConnection());

    expect(result.current.isConnected).toBe(true);
    expect(result.current.status).toBe('connected');
  });
});