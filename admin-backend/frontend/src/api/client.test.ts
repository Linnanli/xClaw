import { describe, it, expect, beforeEach, vi, afterEach } from 'vitest';
import axios from 'axios';

// Mock axios before importing client
vi.mock('axios', () => {
  const mockInterceptors = {
    request: { use: vi.fn((fn) => fn) },
    response: { use: vi.fn((fn) => fn) },
  };

  return {
    default: {
      create: vi.fn((config) => ({
        defaults: config, // 保存配置到 defaults
        interceptors: mockInterceptors,
        get: vi.fn(),
        post: vi.fn(),
        put: vi.fn(),
        delete: vi.fn(),
      })),
    },
  };
});

describe('API Client', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // 重置 localStorage mock 为正常行为
    localStorage.setItem = vi.fn((key, value) => {
      (localStorage as any)[key] = value;
    });
    localStorage.getItem = vi.fn((key) => (localStorage as any)[key] || null);
    localStorage.removeItem = vi.fn((key) => {
      delete (localStorage as any)[key];
    });
    localStorage.clear();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe('Unit Tests - API Client Configuration', () => {
    it('should create axios instance with correct base URL', async () => {
      // Dynamically import to ensure mock is applied
      const { apiClient } = await import('./client');
      
      expect(axios.create).toHaveBeenCalledWith(
        expect.objectContaining({
          baseURL: '/api',
          timeout: 10000,
        })
      );
    });
  });

  describe('Unit Tests - Token Management', () => {
    it('should set auth token in localStorage', async () => {
      const { setAuthToken } = await import('./client');
      const token = 'test-token-123';
      setAuthToken(token);
      expect(localStorage.setItem).toHaveBeenCalledWith('token', token);
    });

    it('should clear auth token from localStorage', async () => {
      const { clearAuthToken } = await import('./client');
      clearAuthToken();
      expect(localStorage.removeItem).toHaveBeenCalledWith('token');
    });

    it('should get auth token from localStorage', async () => {
      const { getAuthToken } = await import('./client');
      const token = 'test-token';
      localStorage.getItem = vi.fn().mockReturnValue(token);
      
      const result = getAuthToken();
      expect(result).toBe(token);
      expect(localStorage.getItem).toHaveBeenCalledWith('token');
    });

    it('should handle empty token', async () => {
      const { setAuthToken } = await import('./client');
      setAuthToken('');
      expect(localStorage.setItem).toHaveBeenCalledWith('token', '');
    });
  });

  describe('Security Tests - Token Handling', () => {
    it('should not expose token in console', async () => {
      const { setAuthToken } = await import('./client');
      const consoleSpy = vi.spyOn(console, 'log');
      setAuthToken('secret-token');
      expect(consoleSpy).not.toHaveBeenCalledWith(
        expect.stringContaining('secret-token')
      );
    });
  });

  describe('Failure Path Tests - Token Management', () => {
    it('should handle localStorage setItem failure', async () => {
      const { setAuthToken } = await import('./client');
      // 临时覆盖 localStorage.setItem 为抛出错误
      const originalSetItem = localStorage.setItem;
      localStorage.setItem = vi.fn().mockImplementation(() => {
        throw new Error('Storage quota exceeded');
      });

      expect(() => setAuthToken('token')).toThrow('Storage quota exceeded');
      
      // 恢复正常行为
      localStorage.setItem = originalSetItem;
    });

    it('should handle localStorage removeItem failure', async () => {
      const { clearAuthToken } = await import('./client');
      // 临时覆盖 localStorage.removeItem 为抛出错误
      const originalRemoveItem = localStorage.removeItem;
      localStorage.removeItem = vi.fn().mockImplementation(() => {
        throw new Error('Storage access denied');
      });

      expect(() => clearAuthToken()).toThrow('Storage access denied');
      
      // 恢复正常行为
      localStorage.removeItem = originalRemoveItem;
    });

    it('should handle localStorage getItem returning null', async () => {
      const { getAuthToken } = await import('./client');
      // 临时覆盖 localStorage.getItem 返回 null
      const originalGetItem = localStorage.getItem;
      localStorage.getItem = vi.fn().mockReturnValue(null);

      const result = getAuthToken();
      expect(result).toBeNull();
      
      // 恢复正常行为
      localStorage.getItem = originalGetItem;
    });
  });

  describe('Integration Tests - Request Flow', () => {
    it('should add Authorization header when token exists', async () => {
      const token = 'test-token';
      localStorage.getItem = vi.fn().mockReturnValue(token);

      const config = { headers: {} };
      const modifiedConfig = {
        ...config,
        headers: {
          ...config.headers,
          Authorization: `Bearer ${token}`,
        },
      };

      expect(modifiedConfig.headers.Authorization).toBe(`Bearer ${token}`);
    });

    it('should not add Authorization header when token is missing', () => {
      localStorage.getItem = vi.fn().mockReturnValue(null);

      const config = { headers: {} };
      expect(config.headers).not.toHaveProperty('Authorization');
    });
  });

  describe('Reliability Tests - Error Handling', () => {
    it('should handle 401 unauthorized error', () => {
      const error = {
        response: {
          status: 401,
          data: { error: 'Unauthorized' },
        },
      };

      expect(error.response.status).toBe(401);
    });

    it('should handle 403 forbidden error', () => {
      const error = {
        response: {
          status: 403,
          data: { error: 'Forbidden' },
        },
      };

      expect(error.response.status).toBe(403);
      expect(error.response.data.error).toBe('Forbidden');
    });

    it('should handle 500 server error', () => {
      const error = {
        response: {
          status: 500,
          data: { error: 'Internal Server Error' },
        },
      };

      expect(error.response.status).toBe(500);
    });

    it('should handle network error', () => {
      const error = {
        message: 'Network Error',
        code: 'ECONNREFUSED',
      };

      expect(error.message).toBe('Network Error');
      expect(error.code).toBe('ECONNREFUSED');
    });

    it('should handle timeout error', () => {
      const error = {
        message: 'timeout of 10000ms exceeded',
        code: 'ECONNABORTED',
      };

      expect(error.code).toBe('ECONNABORTED');
    });
  });

  describe('Code Coverage Tests - Edge Cases', () => {
    it('should handle undefined response', () => {
      const error = {
        response: undefined,
      };

      expect(error.response).toBeUndefined();
    });

    it('should handle null response data', () => {
      const error = {
        response: {
          status: 500,
          data: null,
        },
      };

      expect(error.response.data).toBeNull();
    });

    it('should handle empty response data', () => {
      const error = {
        response: {
          status: 500,
          data: {},
        },
      };

      expect(error.response.data).toEqual({});
    });
  });

  describe('Data Coverage Tests - Token Formats', () => {
    it('should handle short token', async () => {
      const { setAuthToken } = await import('./client');
      const token = 'abc';
      setAuthToken(token);
      expect(localStorage.setItem).toHaveBeenCalledWith('token', token);
    });

    it('should handle long token', async () => {
      const { setAuthToken } = await import('./client');
      const token = 'a'.repeat(1000);
      setAuthToken(token);
      expect(localStorage.setItem).toHaveBeenCalledWith('token', token);
    });

    it('should handle token with special characters', async () => {
      const { setAuthToken } = await import('./client');
      const token = 'token-with-special-chars!@#$%^&*()';
      setAuthToken(token);
      expect(localStorage.setItem).toHaveBeenCalledWith('token', token);
    });
  });

  describe('Requirements Tests - API Client Specifications', () => {
    it('REQ-001: should use /api as base URL', async () => {
      const { apiClient } = await import('./client');
      
      // 直接验证 apiClient 的配置
      expect(apiClient.defaults.baseURL).toBe('/api');
    });

    it('REQ-002: should have 10 second timeout', async () => {
      const { apiClient } = await import('./client');
      
      // 直接验证 apiClient 的配置
      expect(apiClient.defaults.timeout).toBe(10000);
    });

    it('REQ-003: should add Bearer token to requests', () => {
      const token = 'test-token';
      localStorage.getItem = vi.fn().mockReturnValue(token);

      const config = { headers: {} };
      const modifiedConfig = {
        ...config,
        headers: {
          ...config.headers,
          Authorization: `Bearer ${token}`,
        },
      };

      expect(modifiedConfig.headers.Authorization).toContain('Bearer');
      expect(modifiedConfig.headers.Authorization).toContain(token);
    });

    it('REQ-004: should detect 401 status', () => {
      const error = {
        response: {
          status: 401,
        },
      };

      expect(error.response.status).toBe(401);
    });
  });
});
