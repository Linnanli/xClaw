/**
 * useAiChatTauri DLP 集成测试
 *
 * 覆盖维度：DLP 扫描正常路径、失败路径（阻止/降级）、契约测试、安全审计
 * 验证 DLP 扫描结果正确暴露给 UI 层
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useAiChatTauri } from '../useAiChatTauri';
import type { SanitizationResult } from '../useDlpScan';

// ============================================================================
// Mock 依赖
// ============================================================================

const mockScanUserInput = vi.fn<(content: string) => Promise<SanitizationResult>>();

vi.mock('../useDlpScan', () => ({
  useDlpScan: () => ({
    scanUserInput: mockScanUserInput,
    getDlpConfig: vi.fn(),
    updateDlpConfig: vi.fn(),
    getDlpStatistics: vi.fn(),
    scanOutboundRequest: vi.fn(),
    sanitizeForStorage: vi.fn(),
    checkHttpRequest: vi.fn(),
  }),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue({ message_id: 'msg-1', success: true }),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(vi.fn()),
}));

vi.mock('@utils/tracing', () => ({
  tracing: {
    debug: vi.fn(),
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  },
}));

// ============================================================================
// 测试数据工厂
// ============================================================================

function createCleanResult(content: string): SanitizationResult {
  return {
    had_sensitive_data: false,
    sanitized_content: content,
    was_blocked: false,
    sanitization_stats: {
      total_matches: 0,
      redacted_count: 0,
      blocked_count: 0,
      warned_count: 0,
    },
  };
}

function createRedactedResult(sanitized: string): SanitizationResult {
  return {
    had_sensitive_data: true,
    sanitized_content: sanitized,
    was_blocked: false,
    sanitization_stats: {
      total_matches: 2,
      redacted_count: 2,
      blocked_count: 0,
      warned_count: 0,
    },
  };
}

function createBlockedResult(reason: string): SanitizationResult {
  return {
    had_sensitive_data: true,
    sanitized_content: '',
    was_blocked: true,
    block_reason: reason,
    sanitization_stats: {
      total_matches: 1,
      redacted_count: 0,
      blocked_count: 1,
      warned_count: 0,
    },
  };
}

// ============================================================================
// DLP 正常路径
// ============================================================================

describe('useAiChatTauri DLP - 正常路径', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('无敏感数据时 dlpWarning 应为 null', async () => {
    mockScanUserInput.mockResolvedValue(createCleanResult('hello'));

    const { result } = renderHook(() =>
      useAiChatTauri({ threadId: 'thread-1' }),
    );

    await act(async () => {
      await result.current.sendMessage('hello');
    });

    expect(result.current.dlpWarning).toBeNull();
  });

  it('检测到脱敏时应设置 redacted 类型的 dlpWarning', async () => {
    mockScanUserInput.mockResolvedValue(
      createRedactedResult('我的手机号是 138*****000'),
    );

    const { result } = renderHook(() =>
      useAiChatTauri({ threadId: 'thread-1' }),
    );

    await act(async () => {
      await result.current.sendMessage('我的手机号是 13800138000');
    });

    expect(result.current.dlpWarning).not.toBeNull();
    expect(result.current.dlpWarning?.type).toBe('redacted');
    expect(result.current.dlpWarning?.stats.redacted_count).toBe(2);
  });

  it('脱敏后消息应使用 sanitized_content', async () => {
    mockScanUserInput.mockResolvedValue(
      createRedactedResult('身份证 110************234'),
    );

    const { result } = renderHook(() =>
      useAiChatTauri({ threadId: 'thread-1' }),
    );

    await act(async () => {
      await result.current.sendMessage('身份证 110101199003071234');
    });

    const userMsg = result.current.messages.find((m) => m.role === 'user');
    expect(userMsg?.content).toBe('身份证 110************234');
  });

  it('脱敏后消息应携带 dlpStats', async () => {
    mockScanUserInput.mockResolvedValue(
      createRedactedResult('手机 138*****000'),
    );

    const { result } = renderHook(() =>
      useAiChatTauri({ threadId: 'thread-1' }),
    );

    await act(async () => {
      await result.current.sendMessage('手机 13800138000');
    });

    const userMsg = result.current.messages.find((m) => m.role === 'user');
    expect(userMsg?.dlpStats).toBeDefined();
    expect(userMsg?.dlpStats?.redacted_count).toBe(2);
  });

  it('clearDlpWarning 应清除警告', async () => {
    mockScanUserInput.mockResolvedValue(
      createRedactedResult('脱敏内容'),
    );

    const { result } = renderHook(() =>
      useAiChatTauri({ threadId: 'thread-1' }),
    );

    await act(async () => {
      await result.current.sendMessage('敏感内容');
    });

    expect(result.current.dlpWarning).not.toBeNull();

    act(() => {
      result.current.clearDlpWarning();
    });

    expect(result.current.dlpWarning).toBeNull();
  });
});

// ============================================================================
// DLP 失败路径（阻止）
// ============================================================================

describe('useAiChatTauri DLP - 阻止路径', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('消息被阻止时应设置 blocked 类型的 dlpWarning', async () => {
    mockScanUserInput.mockResolvedValue(
      createBlockedResult('检测到 API 密钥'),
    );

    const { result } = renderHook(() =>
      useAiChatTauri({ threadId: 'thread-1' }),
    );

    await act(async () => {
      await result.current.sendMessage('LTAI4G8aB9cD2eFgH3iJ');
    });

    expect(result.current.dlpWarning).not.toBeNull();
    expect(result.current.dlpWarning?.type).toBe('blocked');
    expect(result.current.dlpWarning?.blockReason).toBe('检测到 API 密钥');
  });

  it('消息被阻止时不应添加到消息列表', async () => {
    mockScanUserInput.mockResolvedValue(
      createBlockedResult('检测到私钥'),
    );

    const { result } = renderHook(() =>
      useAiChatTauri({ threadId: 'thread-1' }),
    );

    await act(async () => {
      await result.current.sendMessage('-----BEGIN PRIVATE KEY-----');
    });

    expect(result.current.messages).toHaveLength(0);
  });

  it('消息被阻止时 isLoading 应为 false', async () => {
    mockScanUserInput.mockResolvedValue(
      createBlockedResult('blocked'),
    );

    const { result } = renderHook(() =>
      useAiChatTauri({ threadId: 'thread-1' }),
    );

    await act(async () => {
      await result.current.sendMessage('secret');
    });

    expect(result.current.isLoading).toBe(false);
  });

  it('test_failure_blocked_without_reason', async () => {
    mockScanUserInput.mockResolvedValue({
      had_sensitive_data: true,
      sanitized_content: '',
      was_blocked: true,
      sanitization_stats: {
        total_matches: 1,
        redacted_count: 0,
        blocked_count: 1,
        warned_count: 0,
      },
    });

    const { result } = renderHook(() =>
      useAiChatTauri({ threadId: 'thread-1' }),
    );

    await act(async () => {
      await result.current.sendMessage('secret');
    });

    expect(result.current.dlpWarning?.type).toBe('blocked');
    expect(result.current.dlpWarning?.blockReason).toBeUndefined();
  });
});

// ============================================================================
// DLP 失败路径（扫描异常）
// ============================================================================

describe('useAiChatTauri DLP - 扫描异常', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('test_failure_scan_throws_sets_error', async () => {
    mockScanUserInput.mockRejectedValue(new Error('DLP service unavailable'));

    const onError = vi.fn();
    const { result } = renderHook(() =>
      useAiChatTauri({ threadId: 'thread-1', onError }),
    );

    await act(async () => {
      await result.current.sendMessage('test');
    });

    expect(result.current.error).toContain('DLP service unavailable');
    expect(onError).toHaveBeenCalled();
  });

  it('test_failure_scan_throws_does_not_send_message', async () => {
    mockScanUserInput.mockRejectedValue(new Error('scan failed'));

    const { result } = renderHook(() =>
      useAiChatTauri({ threadId: 'thread-1' }),
    );

    await act(async () => {
      await result.current.sendMessage('test');
    });

    expect(result.current.messages).toHaveLength(0);
  });
});

// ============================================================================
// 契约测试
// ============================================================================

describe('useAiChatTauri DLP - 契约测试', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockScanUserInput.mockResolvedValue(createCleanResult('test'));
  });

  it('test_contract_dlpWarning_initial_null', () => {
    const { result } = renderHook(() =>
      useAiChatTauri({ threadId: 'thread-1' }),
    );
    expect(result.current.dlpWarning).toBeNull();
  });

  it('test_contract_clearDlpWarning_is_function', () => {
    const { result } = renderHook(() =>
      useAiChatTauri({ threadId: 'thread-1' }),
    );
    expect(typeof result.current.clearDlpWarning).toBe('function');
  });

  it('test_contract_dlpWarning_has_timestamp', async () => {
    mockScanUserInput.mockResolvedValue(
      createRedactedResult('脱敏'),
    );

    const { result } = renderHook(() =>
      useAiChatTauri({ threadId: 'thread-1' }),
    );

    await act(async () => {
      await result.current.sendMessage('敏感');
    });

    expect(result.current.dlpWarning?.timestamp).toBeGreaterThan(0);
  });
});

// ============================================================================
// 安全审计测试
// ============================================================================

describe('useAiChatTauri DLP - 安全审计', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('test_audit_original_content_not_in_messages', async () => {
    const originalContent = '身份证 110101199003071234';
    mockScanUserInput.mockResolvedValue(
      createRedactedResult('身份证 110************234'),
    );

    const { result } = renderHook(() =>
      useAiChatTauri({ threadId: 'thread-1' }),
    );

    await act(async () => {
      await result.current.sendMessage(originalContent);
    });

    const userMsg = result.current.messages.find((m) => m.role === 'user');
    expect(userMsg?.content).not.toContain('110101199003071234');
    expect(userMsg?.content).toContain('110************234');
  });

  it('test_audit_blocked_content_not_stored', async () => {
    mockScanUserInput.mockResolvedValue(
      createBlockedResult('API key detected'),
    );

    const { result } = renderHook(() =>
      useAiChatTauri({ threadId: 'thread-1' }),
    );

    await act(async () => {
      await result.current.sendMessage('LTAI4G8aB9cD2eFgH3iJ');
    });

    // 被阻止的消息不应出现在消息列表中
    expect(result.current.messages).toHaveLength(0);
  });

  it('test_audit_dlpWarning_does_not_contain_original_content', async () => {
    mockScanUserInput.mockResolvedValue(
      createRedactedResult('手机 138*****000'),
    );

    const { result } = renderHook(() =>
      useAiChatTauri({ threadId: 'thread-1' }),
    );

    await act(async () => {
      await result.current.sendMessage('手机 13800138000');
    });

    // dlpWarning 只包含统计信息，不包含原始内容
    const warning = result.current.dlpWarning;
    expect(JSON.stringify(warning)).not.toContain('13800138000');
  });

  it('test_audit_error_does_not_leak_sensitive_content', async () => {
    mockScanUserInput.mockRejectedValue(
      new Error('scan failed for content'),
    );

    const { result } = renderHook(() =>
      useAiChatTauri({ threadId: 'thread-1' }),
    );

    await act(async () => {
      await result.current.sendMessage('AKIA1234567890ABCDEF');
    });

    // 错误信息不应包含原始敏感内容
    expect(result.current.error).not.toContain('AKIA1234567890ABCDEF');
  });
});
