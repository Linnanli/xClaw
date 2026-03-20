/**
 * API 配置
 * 统一管理 Desktop Client 的 API 端点配置
 */

/**
 * Desktop Client 嵌入式服务器端口
 * 与 desktop-client/src/embedded_server.rs 中的 EMBEDDED_SERVER_PORT 保持一致
 */
export const EMBEDDED_SERVER_PORT = 38080;

/**
 * API 基础 URL
 * Desktop Client 使用嵌入式 IronClaw 服务器,运行在端口 38080
 */
export const API_BASE_URL = `http://localhost:${EMBEDDED_SERVER_PORT}`;

/**
 * API 端点
 */
export const API_ENDPOINTS = {
  // 聊天相关
  CHAT_SEND: `${API_BASE_URL}/api/chat/send`,
  CHAT_EVENTS: `${API_BASE_URL}/api/chat/events`,
  CHAT_HISTORY: `${API_BASE_URL}/api/chat/history`,
  CHAT_THREADS: `${API_BASE_URL}/api/chat/threads`,
  
  // 日志相关
  LOGS_EVENTS: `${API_BASE_URL}/api/logs/events`,
  
  // 健康检查
  HEALTH: `${API_BASE_URL}/api/health`,
} as const;

/**
 * 获取 API 基础 URL
 * 支持通过环境变量覆盖默认配置
 */
export function getApiBaseUrl(): string {
  // 优先使用环境变量
  if (typeof window !== 'undefined' && (window as any).__API_BASE_URL__) {
    return (window as any).__API_BASE_URL__;
  }
  
  // 使用默认配置
  return API_BASE_URL;
}

/**
 * 构建完整的 API URL
 */
export function buildApiUrl(path: string): string {
  const baseUrl = getApiBaseUrl();
  return `${baseUrl}${path}`;
}
