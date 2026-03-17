// Token Manager - 管理认证令牌
// 支持从 Tauri 命令、URL、本地存储获取令牌

import { invoke } from '@tauri-apps/api/core';

export class TokenManager {
  private static readonly TOKEN_STORAGE_KEY = 'gateway_auth_token';
  private static readonly TOKEN_URL_PARAM = 'token';
  private static cachedToken: string | null = null;

  /**
   * 获取认证令牌
   * 优先级: URL 参数 > Tauri 命令 > 本地存储
   */
  static async getToken(): Promise<string> {
    // 如果有缓存，直接返回
    if (this.cachedToken) {
      return this.cachedToken;
    }

    // 1. 尝试从 URL 参数获取
    const urlToken = this.getTokenFromUrl();
    if (urlToken) {
      console.log('📌 从 URL 获取令牌');
      this.cachedToken = urlToken;
      this.saveToken(urlToken);
      return urlToken;
    }

    // 2. 尝试从 Tauri 命令获取（从后端数据库读取）
    try {
      const tauriToken = await invoke<string>('get_auth_token');
      if (tauriToken && tauriToken.length > 0) {
        console.log('📌 从 Tauri 命令获取令牌（后端数据库）');
        this.cachedToken = tauriToken;
        this.saveToken(tauriToken);
        return tauriToken;
      }
    } catch (err) {
      console.warn('⚠️  无法从 Tauri 命令获取令牌:', err);
      // 继续尝试其他方式，不要立即抛出错误
    }

    // 3. 尝试从本地存储获取
    const storedToken = this.getTokenFromStorage();
    if (storedToken) {
      console.log('📌 从本地存储获取令牌');
      this.cachedToken = storedToken;
      return storedToken;
    }

    // 4. 如果都失败了，抛出详细的错误信息
    throw new Error('Failed to get auth token: No token available from URL, backend database, or local storage. Please ensure the backend is running and properly configured.');
  }

  /**
   * 同步获取令牌（用于非异步上下文）
   * 注意：这只能返回缓存的令牌或本地存储的令牌
   */
  static getTokenSync(): string {
    // 1. 返回缓存的令牌
    if (this.cachedToken) {
      return this.cachedToken;
    }

    // 2. 尝试从 URL 参数获取
    const urlToken = this.getTokenFromUrl();
    if (urlToken) {
      this.cachedToken = urlToken;
      this.saveToken(urlToken);
      return urlToken;
    }

    // 3. 尝试从本地存储获取
    const storedToken = this.getTokenFromStorage();
    if (storedToken) {
      this.cachedToken = storedToken;
      return storedToken;
    }

    // 4. 抛出错误而不是返回空字符串
    throw new Error('Token not available synchronously. Please ensure backend is running or use getToken() async method.');
  }

  /**
   * 从 URL 参数获取令牌
   */
  private static getTokenFromUrl(): string | null {
    try {
      const params = new URLSearchParams(window.location.search);
      return params.get(this.TOKEN_URL_PARAM);
    } catch (err) {
      console.error('Failed to get token from URL:', err);
      return null;
    }
  }

  /**
   * 从本地存储获取令牌
   */
  private static getTokenFromStorage(): string | null {
    try {
      return localStorage.getItem(this.TOKEN_STORAGE_KEY);
    } catch (err) {
      console.error('Failed to get token from storage:', err);
      return null;
    }
  }

  /**
   * 保存令牌到本地存储
   */
  static saveToken(token: string): void {
    try {
      this.cachedToken = token;
      localStorage.setItem(this.TOKEN_STORAGE_KEY, token);
      console.log('✅ 令牌已保存到本地存储');
    } catch (err) {
      console.error('Failed to save token to storage:', err);
    }
  }

  /**
   * 清除本地存储的令牌和缓存
   */
  static clearToken(): void {
    try {
      this.cachedToken = null;
      localStorage.removeItem(this.TOKEN_STORAGE_KEY);
      console.log('✅ 令牌已清除');
    } catch (err) {
      console.error('Failed to clear token:', err);
    }
  }

  /**
   * 检查令牌是否有效
   */
  static isTokenValid(token: string): boolean {
    return token && token.length === 64 && /^[0-9a-fA-F]+$/.test(token);
  }

  /**
   * 刷新令牌（从后端重新获取）
   */
  static async refreshToken(): Promise<string> {
    console.log('🔄 刷新令牌...');
    
    // 清除缓存，强制重新获取
    this.cachedToken = null;
    
    try {
      // 尝试从 Tauri 命令获取新令牌
      const newToken = await invoke<string>('get_auth_token');
      if (newToken && this.isTokenValid(newToken)) {
        console.log('✅ 令牌刷新成功');
        this.cachedToken = newToken;
        this.saveToken(newToken);
        return newToken;
      } else {
        throw new Error('Invalid token received from backend');
      }
    } catch (err) {
      console.error('❌ 令牌刷新失败:', err);
      throw new Error(`Failed to refresh token: ${err instanceof Error ? err.message : 'Unknown error'}`);
    }
  }

  /**
   * 尝试获取令牌的内部方法（用于重试逻辑）
   */
  private static async tryGetToken(): Promise<string | null> {
    // 1. 尝试从 URL 参数获取
    const urlToken = this.getTokenFromUrl();
    if (urlToken && this.isTokenValid(urlToken)) {
      return urlToken;
    }

    // 2. 尝试从 Tauri 命令获取
    try {
      const tauriToken = await invoke<string>('get_auth_token');
      if (tauriToken && this.isTokenValid(tauriToken)) {
        return tauriToken;
      }
    } catch (err) {
      console.warn('Failed to get token from Tauri command:', err);
    }

    // 3. 尝试从本地存储获取
    const storedToken = this.getTokenFromStorage();
    if (storedToken && this.isTokenValid(storedToken)) {
      return storedToken;
    }

    return null;
  }

  /**
   * 获取令牌摘要（用于日志）
   */
  static getTokenSummary(token: string): string {
    if (!token || token.length < 8) {
      return 'invalid';
    }
    return `${token.substring(0, 8)}...${token.substring(token.length - 8)}`;
  }
}
