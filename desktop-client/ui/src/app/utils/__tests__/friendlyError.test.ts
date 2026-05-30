/**
 * friendlyError 单元测试
 *
 * 覆盖维度：
 * - 契约测试：用真实错误格式固定解析行为
 * - 失败路径：无法识别的错误返回通用提示
 * - 安全审计：不泄露技术细节给用户
 */

import { describe, it, expect } from 'vitest';
import { isErrorResponse, friendlyErrorMessage } from '../friendlyError';

// ── 契约测试：真实错误格式 ──────────────────────────────────────

describe('friendlyErrorMessage - 契约测试', () => {
  it('test_contract_real_free_tier_exhausted', () => {
    const raw =
      'Error: LLM error: Provider qwen-max request failed: ' +
      'HttpError: Invalid status code 403 Forbidden with message: ' +
      '{"error":{"message":"The free tier of the model has been exhausted.","type":"AllocationQuota.FreeTierOnly","param":null,"code":"AllocationQuota.FreeTierOnly"},"id":"chatcmpl-e4757e7e","request_id":"e4757e7e"}';

    expect(isErrorResponse(raw)).toBe(true);
    const msg = friendlyErrorMessage(raw);
    expect(msg).toContain('免费额度已用完');
    expect(msg).toContain('qwen-max');
    expect(msg).toContain('控制台');
    // 安全审计：不泄露技术细节
    expect(msg).not.toContain('HttpError');
    expect(msg).not.toContain('AllocationQuota');
    expect(msg).not.toContain('chatcmpl');
  });

  it('test_contract_real_generic_403', () => {
    const raw = 'Error: LLM error: Provider openai request failed: HttpError: Invalid status code 403 Forbidden with message: Forbidden';
    const msg = friendlyErrorMessage(raw);
    expect(msg).toContain('403');
    expect(msg).toContain('openai');
  });

  it('test_contract_real_401', () => {
    const raw = 'Error: LLM error: Provider anthropic request failed: HttpError: Invalid status code 401 Unauthorized with message: Unauthorized';
    const msg = friendlyErrorMessage(raw);
    expect(msg).toContain('认证失败');
    expect(msg).toContain('API Key');
  });

  it('test_contract_real_429', () => {
    const raw = 'Error: LLM error: Provider qwen-max request failed: HttpError: Invalid status code 429 Too Many Requests with message: Rate limited';
    const msg = friendlyErrorMessage(raw);
    expect(msg).toContain('频率超限');
  });

  it('test_contract_real_503', () => {
    const raw = 'Error: LLM error: Provider openai request failed: HttpError: Invalid status code 503 Service Unavailable with message: Service Unavailable';
    const msg = friendlyErrorMessage(raw);
    expect(msg).toContain('暂时不可用');
  });
});

// ── 兼容性测试：旧格式 ──────────────────────────────────────────

describe('friendlyErrorMessage - 旧格式兼容', () => {
  it('test_compat_old_http_format', () => {
    const raw = 'Error: LLM error: Provider qwen-max request failed: HTTP 403: {"error":{"type":"AllocationQuota.FreeTierOnly"}}';
    const msg = friendlyErrorMessage(raw);
    expect(msg).toContain('免费额度已用完');
  });
});

// ── 失败路径测试 ────────────────────────────────────────────────

describe('friendlyErrorMessage - 失败路径', () => {
  it('test_failure_unrecognized_error', () => {
    const msg = friendlyErrorMessage('Error: some internal panic');
    expect(msg).toContain('稍后重试');
    expect(msg).not.toContain('panic');
  });

  it('test_failure_empty_string', () => {
    const msg = friendlyErrorMessage('');
    expect(msg.length).toBeGreaterThan(0);
    expect(msg).toContain('稍后重试');
  });

  it('test_failure_non_error_prefix', () => {
    const msg = friendlyErrorMessage('some random text');
    expect(msg).toContain('稍后重试');
  });
});

// ── isErrorResponse 测试 ────────────────────────────────────────

describe('isErrorResponse', () => {
  it('detects Error: prefix', () => {
    expect(isErrorResponse('Error: something')).toBe(true);
  });

  it('rejects normal content', () => {
    expect(isErrorResponse('Hello world')).toBe(false);
  });

  it('rejects empty string', () => {
    expect(isErrorResponse('')).toBe(false);
  });
});
