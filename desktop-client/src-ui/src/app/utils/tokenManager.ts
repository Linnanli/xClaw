/**
 * Token Manager — 管理认证令牌
 *
 * 优先级: URL 参数 > Tauri 命令 > 本地存储
 */

import { invoke } from '@tauri-apps/api/core';

const TOKEN_STORAGE_KEY = 'gateway_auth_token';
const TOKEN_URL_PARAM = 'token';

let cachedToken: string | null = null;

function getTokenFromUrl(): string | null {
  try {
    const params = new URLSearchParams(window.location.search);
    return params.get(TOKEN_URL_PARAM);
  } catch {
    return null;
  }
}

function getTokenFromStorage(): string | null {
  try {
    return localStorage.getItem(TOKEN_STORAGE_KEY);
  } catch {
    return null;
  }
}

function saveToStorage(token: string): void {
  try {
    localStorage.setItem(TOKEN_STORAGE_KEY, token);
  } catch (err) {
    console.error('Failed to save token to storage:', err);
  }
}

export class TokenManager {
  /**
   * 获取认证令牌（异步，完整优先级链）
   */
  static async getToken(): Promise<string> {
    if (cachedToken) return cachedToken;

    // 1. URL 参数
    const urlToken = getTokenFromUrl();
    if (urlToken) {
      cachedToken = urlToken;
      saveToStorage(urlToken);
      return urlToken;
    }

    // 2. Tauri 命令（后端数据库）
    try {
      const tauriToken = await invoke<string>('get_auth_token');
      if (tauriToken) {
        cachedToken = tauriToken;
        saveToStorage(tauriToken);
        return tauriToken;
      }
    } catch (err) {
      console.warn('Failed to get token from Tauri:', err);
    }

    // 3. 本地存储
    const storedToken = getTokenFromStorage();
    if (storedToken) {
      cachedToken = storedToken;
      return storedToken;
    }

    throw new Error('No auth token available from URL, backend, or local storage.');
  }
}
