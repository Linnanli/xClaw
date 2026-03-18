/**
 * DLP 扫描 Hook
 * 提供敏感信息检测和脱敏功能
 */

import { invoke } from '@tauri-apps/api/core';

export interface SanitizationStats {
  total_matches: number;
  redacted_count: number;
  blocked_count: number;
  warned_count: number;
}

export interface SanitizationResult {
  had_sensitive_data: boolean;
  sanitized_content: string;
  was_blocked: boolean;
  block_reason?: string;
  sanitization_stats: SanitizationStats;
}

export interface DlpConfig {
  enabled: boolean;
  sanitization: {
    redaction_text: string;
    preserve_format: boolean;
    partial_redaction: boolean;
  };
  real_time_monitoring: boolean;
  audit_logging: boolean;
  custom_patterns: Array<{
    name: string;
    pattern: string;
    severity: string;
    action: string;
    description?: string;
    enabled: boolean;
  }>;
}

export interface DlpStatistics {
  total_scans: number;
  sensitive_data_detected: number;
  content_blocked: number;
  content_sanitized: number;
  http_requests_blocked: number;
}

/**
 * DLP 扫描 Hook
 */
export function useDlpScan() {
  /**
   * 扫描用户输入
   */
  const scanUserInput = async (content: string): Promise<SanitizationResult> => {
    try {
      return await invoke<SanitizationResult>('scan_user_input', { content });
    } catch (error) {
      console.error('❌ DLP scan failed:', error);
      throw error;
    }
  };

  /**
   * 扫描出站请求
   */
  const scanOutboundRequest = async (body: string): Promise<SanitizationResult> => {
    try {
      return await invoke<SanitizationResult>('scan_outbound_request', { body });
    } catch (error) {
      console.error('❌ DLP scan failed:', error);
      throw error;
    }
  };

  /**
   * 为存储脱敏内容
   */
  const sanitizeForStorage = async (content: string): Promise<string> => {
    try {
      return await invoke<string>('sanitize_for_storage', { content });
    } catch (error) {
      console.error('❌ DLP sanitization failed:', error);
      throw error;
    }
  };

  /**
   * 检查 HTTP 请求
   */
  const checkHttpRequest = async (
    url: string,
    headers: Array<[string, string]>,
    body?: Uint8Array
  ): Promise<void> => {
    try {
      await invoke('check_http_request', { 
        url, 
        headers,
        body: body ? Array.from(body) : undefined
      });
    } catch (error) {
      console.error('❌ HTTP request blocked by DLP:', error);
      throw error;
    }
  };

  /**
   * 获取 DLP 配置
   */
  const getDlpConfig = async (): Promise<DlpConfig> => {
    try {
      return await invoke<DlpConfig>('get_dlp_config');
    } catch (error) {
      console.error('❌ Failed to get DLP config:', error);
      throw error;
    }
  };

  /**
   * 更新 DLP 配置
   */
  const updateDlpConfig = async (config: DlpConfig): Promise<void> => {
    try {
      await invoke('update_dlp_config', { config });
    } catch (error) {
      console.error('❌ Failed to update DLP config:', error);
      throw error;
    }
  };

  /**
   * 获取 DLP 统计信息
   */
  const getDlpStatistics = async (): Promise<DlpStatistics> => {
    try {
      return await invoke<DlpStatistics>('get_dlp_statistics');
    } catch (error) {
      console.error('❌ Failed to get DLP statistics:', error);
      throw error;
    }
  };

  return {
    scanUserInput,
    scanOutboundRequest,
    sanitizeForStorage,
    checkHttpRequest,
    getDlpConfig,
    updateDlpConfig,
    getDlpStatistics,
  };
}
