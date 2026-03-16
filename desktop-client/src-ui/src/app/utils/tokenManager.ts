// Token Manager - 管理认证令牌
// 支持从 URL、本地存储或环境变量获取令牌

export class TokenManager {
  private static readonly TOKEN_STORAGE_KEY = 'gateway_auth_token';
  private static readonly TOKEN_URL_PARAM = 'token';

  /**
   * 获取认证令牌
   * 优先级: URL 参数 > 本地存储 > 默认值
   */
  static getToken(): string {
    // 1. 尝试从 URL 参数获取
    const urlToken = this.getTokenFromUrl();
    if (urlToken) {
      console.log('📌 从 URL 获取令牌');
      this.saveToken(urlToken);
      return urlToken;
    }

    // 2. 尝试从本地存储获取
    const storedToken = this.getTokenFromStorage();
    if (storedToken) {
      console.log('📌 从本地存储获取令牌');
      return storedToken;
    }

    // 3. 返回默认值（应该在启动脚本中更新）
    console.warn('⚠️  使用默认令牌，请确保后端已启动');
    return '36c1a0275ae2708954a402871a27255bcecf9aa24f7542a43ff0c5e1f651b19b';
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
      localStorage.setItem(this.TOKEN_STORAGE_KEY, token);
      console.log('✅ 令牌已保存到本地存储');
    } catch (err) {
      console.error('Failed to save token to storage:', err);
    }
  }

  /**
   * 清除本地存储的令牌
   */
  static clearToken(): void {
    try {
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
    return token && token.length > 0;
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
