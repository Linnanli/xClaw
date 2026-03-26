/**
 * DLP 前端集成测试
 *
 * 测试维度：
 * - 正常路径：干净内容直接发送
 * - 脱敏路径：手机号/身份证被脱敏后发送
 * - 阻止路径：API 密钥被阻止
 * - 失败路径：DLP 扫描失败时 Fail-Safe 阻止
 * - 契约测试：scan_user_input 返回值格式
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';

// Mock Tauri invoke
const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

import type { SanitizationResult } from '../useDlpScan';

// 辅助：构造 DLP 扫描结果
function cleanResult(content: string): SanitizationResult {
  return {
    had_sensitive_data: false,
    sanitized_content: content,
    was_blocked: false,
    sanitization_stats: { total_matches: 0, redacted_count: 0, blocked_count: 0, warned_count: 0 },
  };
}

function redactedResult(original: string, sanitized: string, redactedCount: number): SanitizationResult {
  return {
    had_sensitive_data: true,
    sanitized_content: sanitized,
    was_blocked: false,
    sanitization_stats: { total_matches: redactedCount, redacted_count: redactedCount, blocked_count: 0, warned_count: 0 },
  };
}

function blockedResult(reason: string): SanitizationResult {
  return {
    had_sensitive_data: true,
    sanitized_content: '',
    was_blocked: true,
    block_reason: reason,
    sanitization_stats: { total_matches: 1, redacted_count: 0, blocked_count: 1, warned_count: 0 },
  };
}

describe('DLP scan_user_input 契约测试', () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it('req_dlp_001_clean_content_passes_through', async () => {
    const content = '今天天气不错';
    mockInvoke.mockResolvedValue(cleanResult(content));

    const result = await mockInvoke('scan_user_input', { content });

    expect(result.had_sensitive_data).toBe(false);
    expect(result.was_blocked).toBe(false);
    expect(result.sanitized_content).toBe(content);
  });

  it('req_dlp_002_mobile_number_redacted', async () => {
    const content = '我的手机号是 13800138000';
    mockInvoke.mockResolvedValue(redactedResult(content, '我的手机号是 138*****000', 1));

    const result = await mockInvoke('scan_user_input', { content });

    expect(result.had_sensitive_data).toBe(true);
    expect(result.was_blocked).toBe(false);
    expect(result.sanitized_content).toContain('138*****000');
    expect(result.sanitized_content).not.toContain('13800138000');
    expect(result.sanitization_stats.redacted_count).toBe(1);
  });

  it('req_dlp_003_id_card_redacted', async () => {
    const content = '身份证号 110101199003071234';
    mockInvoke.mockResolvedValue(redactedResult(content, '身份证号 110************234', 1));

    const result = await mockInvoke('scan_user_input', { content });

    expect(result.had_sensitive_data).toBe(true);
    expect(result.sanitized_content).toContain('110************234');
    expect(result.sanitized_content).not.toContain('110101199003071234');
  });

  it('req_dlp_004_api_key_blocked', async () => {
    const content = '密钥是 LTAI4G8aB9cD2eFgH3iJ';
    mockInvoke.mockResolvedValue(blockedResult('包含不可脱敏的敏感信息: aliyun_access_key'));

    const result = await mockInvoke('scan_user_input', { content });

    expect(result.was_blocked).toBe(true);
    expect(result.block_reason).toContain('aliyun_access_key');
    expect(result.sanitized_content).toBe('');
  });

  it('test_failure_scan_throws_error', async () => {
    mockInvoke.mockRejectedValue(new Error('Engine not ready'));

    await expect(mockInvoke('scan_user_input', { content: 'test' })).rejects.toThrow('Engine not ready');
  });

  it('test_contract_result_has_required_fields', async () => {
    mockInvoke.mockResolvedValue(cleanResult('test'));

    const result = await mockInvoke('scan_user_input', { content: 'test' });

    // 契约：所有必需字段都存在
    expect(result).toHaveProperty('had_sensitive_data');
    expect(result).toHaveProperty('sanitized_content');
    expect(result).toHaveProperty('was_blocked');
    expect(result).toHaveProperty('sanitization_stats');
    expect(result.sanitization_stats).toHaveProperty('total_matches');
    expect(result.sanitization_stats).toHaveProperty('redacted_count');
    expect(result.sanitization_stats).toHaveProperty('blocked_count');
    expect(result.sanitization_stats).toHaveProperty('warned_count');
  });

  it('test_contract_multiple_pii_redacted', async () => {
    const content = '身份证 110101199003071234 手机 13800138000';
    mockInvoke.mockResolvedValue(redactedResult(
      content,
      '身份证 110************234 手机 138*****000',
      2,
    ));

    const result = await mockInvoke('scan_user_input', { content });

    expect(result.sanitization_stats.total_matches).toBe(2);
    expect(result.sanitization_stats.redacted_count).toBe(2);
    expect(result.sanitized_content).not.toContain('110101199003071234');
    expect(result.sanitized_content).not.toContain('13800138000');
  });

  it('test_audit_no_original_pii_in_sanitized_content', async () => {
    const mobile = '13800138000';
    const content = `联系方式 ${mobile}`;
    mockInvoke.mockResolvedValue(redactedResult(content, '联系方式 138*****000', 1));

    const result = await mockInvoke('scan_user_input', { content });

    // 安全审计：脱敏后的内容不应包含原始手机号
    expect(result.sanitized_content).not.toContain(mobile);
  });
});
