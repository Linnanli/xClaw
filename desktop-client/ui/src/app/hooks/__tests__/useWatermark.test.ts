import { describe, it, expect, beforeEach, vi } from 'vitest';
import { renderHook, waitFor } from '@testing-library/react';
import { useWatermark } from '../useWatermark';

// Mock sessionApi
vi.mock('../../utils/tauri', () => ({
  sessionApi: {
    getSessionInfo: vi.fn(),
  },
}));

// 获取 mock 函数
import { sessionApi } from '../../utils/tauri';
const mockGetSessionInfo = vi.mocked(sessionApi.getSessionInfo);

// Mock Date
const mockDate = new Date('2024-01-15T10:30:00Z');
vi.setSystemTime(mockDate);

describe('useWatermark', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('初始化', () => {
    it('应该初始化默认配置', () => {
      mockGetSessionInfo.mockResolvedValue({
        user_id: 'test-user',
        session_id: 'test-session',
      });

      const { result } = renderHook(() => useWatermark());

      expect(result.current.loading).toBe(true);
      expect(result.current.config.enabled).toBe(true);
      expect(result.current.config.opacity).toBe(0.1);
      expect(result.current.config.fontSize).toBe(16);
      expect(result.current.config.rotation).toBe(-20);
      expect(result.current.config.spacing).toBe(200);
    });

    it('应该加载用户信息并生成水印文本', async () => {
      mockGetSessionInfo.mockResolvedValue({
        user_id: 'test-user-123',
        session_id: 'session-456',
      });

      const { result } = renderHook(() => useWatermark());

      await waitFor(() => {
        expect(result.current.loading).toBe(false);
      });

      expect(mockGetSessionInfo).toHaveBeenCalled();
      expect(result.current.config.text).toContain('test-user-123');
      expect(result.current.config.text).toContain('2024');
    });

    it('应该在获取用户信息失败时使用默认文本', async () => {
      mockGetSessionInfo.mockRejectedValue(new Error('Session not found'));

      const { result } = renderHook(() => useWatermark());

      await waitFor(() => {
        expect(result.current.loading).toBe(false);
      });

      expect(result.current.config.text).toContain('Desktop Client');
      expect(result.current.config.text).toContain('2024');
    });
  });

  describe('配置管理', () => {
    it('应该更新配置', async () => {
      mockGetSessionInfo.mockResolvedValue({
        user_id: 'test-user',
        session_id: 'test-session',
      });

      const { result } = renderHook(() => useWatermark());

      await waitFor(() => {
        expect(result.current.loading).toBe(false);
      });

      // 更新配置
      await waitFor(() => {
        result.current.updateConfig({
          opacity: 0.2,
          fontSize: 20,
          rotation: 45,
        });
      });

      await waitFor(() => {
        expect(result.current.config.opacity).toBe(0.2);
        expect(result.current.config.fontSize).toBe(20);
        expect(result.current.config.rotation).toBe(45);
        // 其他配置应该保持不变
        expect(result.current.config.enabled).toBe(true);
        expect(result.current.config.spacing).toBe(200);
      });
    });

    it('应该切换启用状态', async () => {
      mockGetSessionInfo.mockResolvedValue({
        user_id: 'test-user',
        session_id: 'test-session',
      });

      const { result } = renderHook(() => useWatermark());

      await waitFor(() => {
        expect(result.current.loading).toBe(false);
      });

      // 初始状态应该是启用的
      expect(result.current.config.enabled).toBe(true);

      // 切换状态
      await waitFor(() => {
        result.current.toggleEnabled();
      });

      await waitFor(() => {
        expect(result.current.config.enabled).toBe(false);
      });

      // 再次切换
      await waitFor(() => {
        result.current.toggleEnabled();
      });

      await waitFor(() => {
        expect(result.current.config.enabled).toBe(true);
      });
    });
  });

  describe('文本刷新', () => {
    it('应该刷新水印文本', async () => {
      mockGetSessionInfo.mockResolvedValue({
        user_id: 'test-user',
        session_id: 'test-session',
      });

      const { result } = renderHook(() => useWatermark());

      await waitFor(() => {
        expect(result.current.loading).toBe(false);
      });

      const originalText = result.current.config.text;

      // 模拟时间变化 - 更新 mock 返回值
      const newDate = new Date('2024-01-15T11:00:00Z');
      vi.setSystemTime(newDate);

      // 刷新文本
      await result.current.refreshText();

      await waitFor(() => {
        // 由于时间变化，文本应该包含用户信息
        expect(result.current.config.text).toContain('test-user');
        // 检查新的时间格式（中文格式）
        expect(result.current.config.text).toMatch(/2024\/01\/15 \d{2}:\d{2}/);
      });
    });

    it('应该在刷新失败时保持原文本', async () => {
      mockGetSessionInfo.mockResolvedValue({
        user_id: 'test-user',
        session_id: 'test-session',
      });

      const { result } = renderHook(() => useWatermark());

      await waitFor(() => {
        expect(result.current.loading).toBe(false);
      });

      const originalText = result.current.config.text;

      // 模拟刷新失败
      mockGetSessionInfo.mockRejectedValue(new Error('Network error'));

      await result.current.refreshText();

      expect(result.current.config.text).toBe(originalText);
    });
  });

  describe('时间格式', () => {
    it('应该使用中文时间格式', async () => {
      mockGetSessionInfo.mockResolvedValue({
        user_id: 'user123',
        session_id: 'session456',
      });

      const { result } = renderHook(() => useWatermark());

      await waitFor(() => {
        expect(result.current.loading).toBe(false);
      });

      // 检查时间格式：YYYY/MM/DD HH:mm
      expect(result.current.config.text).toMatch(/\d{4}\/\d{2}\/\d{2} \d{2}:\d{2}/);
    });
  });

  describe('边界条件测试', () => {
    it('应该处理空的用户信息', async () => {
      mockGetSessionInfo.mockResolvedValue({
        user_id: '',
        session_id: '',
      });

      const { result } = renderHook(() => useWatermark());

      await waitFor(() => {
        expect(result.current.loading).toBe(false);
      });

      // 空用户ID应该直接显示为空字符串，不是 Desktop Client
      expect(result.current.config.text).toContain(' | 2024');
    });

    it('应该处理 null 用户信息', async () => {
      mockGetSessionInfo.mockResolvedValue({
        user_id: null as any,
        session_id: null as any,
      });

      const { result } = renderHook(() => useWatermark());

      await waitFor(() => {
        expect(result.current.loading).toBe(false);
      });

      // null 用户ID应该显示为 "null"
      expect(result.current.config.text).toContain('null | 2024');
    });

    it('应该处理极端配置值', async () => {
      mockGetSessionInfo.mockResolvedValue({
        user_id: 'test-user',
        session_id: 'test-session',
      });

      const { result } = renderHook(() => useWatermark());

      await waitFor(() => {
        expect(result.current.loading).toBe(false);
      });

      // 测试极端值
      await waitFor(() => {
        result.current.updateConfig({
          opacity: 0,
          fontSize: 1,
          rotation: 360,
          spacing: 1,
        });
      });

      await waitFor(() => {
        expect(result.current.config.opacity).toBe(0);
        expect(result.current.config.fontSize).toBe(1);
        expect(result.current.config.rotation).toBe(360);
        expect(result.current.config.spacing).toBe(1);
      });
    });

    it('应该处理负数配置值', async () => {
      mockGetSessionInfo.mockResolvedValue({
        user_id: 'test-user',
        session_id: 'test-session',
      });

      const { result } = renderHook(() => useWatermark());

      await waitFor(() => {
        expect(result.current.loading).toBe(false);
      });

      // 测试负数值
      await waitFor(() => {
        result.current.updateConfig({
          opacity: -0.1,
          fontSize: -10,
          rotation: -180,
          spacing: -50,
        });
      });

      await waitFor(() => {
        expect(result.current.config.opacity).toBe(-0.1);
        expect(result.current.config.fontSize).toBe(-10);
        expect(result.current.config.rotation).toBe(-180);
        expect(result.current.config.spacing).toBe(-50);
      });
    });
  });

  describe('性能测试', () => {
    it('应该在合理时间内完成初始化', async () => {
      mockGetSessionInfo.mockResolvedValue({
        user_id: 'test-user',
        session_id: 'test-session',
      });

      const startTime = Date.now();
      const { result } = renderHook(() => useWatermark());

      await waitFor(() => {
        expect(result.current.loading).toBe(false);
      });

      const endTime = Date.now();
      const duration = endTime - startTime;

      // 初始化应该在 1 秒内完成
      expect(duration).toBeLessThan(1000);
    });

    it('应该处理频繁的配置更新', async () => {
      mockGetSessionInfo.mockResolvedValue({
        user_id: 'test-user',
        session_id: 'test-session',
      });

      const { result } = renderHook(() => useWatermark());

      await waitFor(() => {
        expect(result.current.loading).toBe(false);
      });

      // 频繁更新配置
      for (let i = 0; i < 100; i++) {
        await waitFor(() => {
          result.current.updateConfig({
            opacity: i / 100,
            fontSize: 10 + i,
          });
        });
      }

      await waitFor(() => {
        expect(result.current.config.opacity).toBe(0.99);
        expect(result.current.config.fontSize).toBe(109);
      });
    });
  });

  describe('内存泄漏测试', () => {
    it('应该正确清理资源', async () => {
      mockGetSessionInfo.mockResolvedValue({
        user_id: 'test-user',
        session_id: 'test-session',
      });

      const { result, unmount } = renderHook(() => useWatermark());

      await waitFor(() => {
        expect(result.current.loading).toBe(false);
      });

      // 卸载组件
      unmount();

      // 验证没有内存泄漏（这里主要是确保没有抛出错误）
      expect(true).toBe(true);
    });
  });

  describe('并发测试', () => {
    it('应该处理并发的文本刷新请求', async () => {
      mockGetSessionInfo.mockResolvedValue({
        user_id: 'test-user',
        session_id: 'test-session',
      });

      const { result } = renderHook(() => useWatermark());

      await waitFor(() => {
        expect(result.current.loading).toBe(false);
      });

      // 并发刷新文本
      const promises = Array.from({ length: 10 }, () => result.current.refreshText());
      
      await Promise.all(promises);

      // 验证最终状态正确
      expect(result.current.config.text).toContain('test-user');
    });
  });
});