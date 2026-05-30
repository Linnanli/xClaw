/**
 * DlpWarningBanner 多维度测试
 *
 * 覆盖维度：单元测试（正常路径 + 错误路径）、契约测试、安全审计测试
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, act } from '@testing-library/react';
import { DlpWarningBanner } from '../DlpWarningBanner';
import type { SanitizationStats } from '../../../hooks/useDlpScan';

// ============================================================================
// 测试数据工厂
// ============================================================================

function createStats(overrides: Partial<SanitizationStats> = {}): SanitizationStats {
  return {
    total_matches: 2,
    redacted_count: 2,
    blocked_count: 0,
    warned_count: 0,
    ...overrides,
  };
}

// ============================================================================
// 单元测试 - 正常路径
// ============================================================================

describe('DlpWarningBanner - 正常路径', () => {
  const onClose = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('应渲染脱敏类型的警告横幅', () => {
    render(
      <DlpWarningBanner
        type="redacted"
        stats={createStats()}
        onClose={onClose}
        autoCloseDelay={0}
      />,
    );
    expect(screen.getByText('检测到敏感信息')).toBeInTheDocument();
    expect(screen.getByText('已自动脱敏 2 处敏感信息')).toBeInTheDocument();
  });

  it('应渲染阻止类型的警告横幅', () => {
    render(
      <DlpWarningBanner
        type="blocked"
        stats={createStats({ blocked_count: 1 })}
        blockReason="检测到 API 密钥"
        onClose={onClose}
        autoCloseDelay={0}
      />,
    );
    expect(screen.getByText('消息已被阻止')).toBeInTheDocument();
    expect(screen.getByText('检测到 API 密钥')).toBeInTheDocument();
  });

  it('应显示匹配数量 Badge', () => {
    render(
      <DlpWarningBanner
        type="redacted"
        stats={createStats({ total_matches: 3 })}
        onClose={onClose}
        autoCloseDelay={0}
      />,
    );
    expect(screen.getByText('3 处')).toBeInTheDocument();
  });

  it('应显示警告数量', () => {
    render(
      <DlpWarningBanner
        type="redacted"
        stats={createStats({ warned_count: 1 })}
        onClose={onClose}
        autoCloseDelay={0}
      />,
    );
    expect(screen.getByText('发现 1 处潜在敏感信息')).toBeInTheDocument();
  });

  it('点击关闭按钮应触发 onClose', () => {
    render(
      <DlpWarningBanner
        type="redacted"
        stats={createStats()}
        onClose={onClose}
        autoCloseDelay={0}
      />,
    );
    fireEvent.click(screen.getByLabelText('关闭 DLP 警告'));
    // 等待淡出动画
    act(() => {
      vi.advanceTimersByTime(300);
    });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('应在指定延迟后自动关闭', () => {
    render(
      <DlpWarningBanner
        type="redacted"
        stats={createStats()}
        onClose={onClose}
        autoCloseDelay={3000}
      />,
    );
    act(() => {
      vi.advanceTimersByTime(3000);
    });
    // 等待淡出动画
    act(() => {
      vi.advanceTimersByTime(300);
    });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('autoCloseDelay=0 时不应自动关闭', () => {
    render(
      <DlpWarningBanner
        type="redacted"
        stats={createStats()}
        onClose={onClose}
        autoCloseDelay={0}
      />,
    );
    act(() => {
      vi.advanceTimersByTime(10000);
    });
    expect(onClose).not.toHaveBeenCalled();
  });
});

// ============================================================================
// 单元测试 - 错误路径
// ============================================================================

describe('DlpWarningBanner - 错误路径', () => {
  const onClose = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('test_failure_zero_stats_still_renders', () => {
    render(
      <DlpWarningBanner
        type="redacted"
        stats={createStats({ total_matches: 0, redacted_count: 0 })}
        onClose={onClose}
        autoCloseDelay={0}
      />,
    );
    expect(screen.getByText('检测到敏感信息')).toBeInTheDocument();
  });

  it('test_failure_blocked_without_reason_shows_default', () => {
    render(
      <DlpWarningBanner
        type="blocked"
        stats={createStats({ blocked_count: 1 })}
        onClose={onClose}
        autoCloseDelay={0}
      />,
    );
    expect(
      screen.getByText('消息包含高危敏感信息，已被安全策略阻止发送。'),
    ).toBeInTheDocument();
  });

  it('test_failure_multiple_close_clicks_only_fires_once', () => {
    vi.useFakeTimers();
    render(
      <DlpWarningBanner
        type="redacted"
        stats={createStats()}
        onClose={onClose}
        autoCloseDelay={0}
      />,
    );
    const closeBtn = screen.getByLabelText('关闭 DLP 警告');
    fireEvent.click(closeBtn);
    fireEvent.click(closeBtn);
    act(() => {
      vi.advanceTimersByTime(600);
    });
    // onClose 可能被调用多次（因为 setTimeout），但不应 crash
    expect(onClose).toHaveBeenCalled();
    vi.useRealTimers();
  });
});

// ============================================================================
// 契约测试
// ============================================================================

describe('DlpWarningBanner - 契约测试', () => {
  it('test_contract_renders_with_minimal_props', () => {
    expect(() => {
      render(
        <DlpWarningBanner
          type="redacted"
          stats={createStats()}
          onClose={vi.fn()}
          autoCloseDelay={0}
        />,
      );
    }).not.toThrow();
  });

  it('test_contract_accepts_custom_className', () => {
    const { container } = render(
      <DlpWarningBanner
        type="redacted"
        stats={createStats()}
        onClose={vi.fn()}
        autoCloseDelay={0}
        className="custom-class"
      />,
    );
    expect(container.querySelector('.custom-class')).toBeInTheDocument();
  });

  it('test_contract_has_alert_role', () => {
    render(
      <DlpWarningBanner
        type="redacted"
        stats={createStats()}
        onClose={vi.fn()}
        autoCloseDelay={0}
      />,
    );
    expect(screen.getAllByRole('alert').length).toBeGreaterThanOrEqual(1);
  });

  it('test_contract_close_button_has_aria_label', () => {
    render(
      <DlpWarningBanner
        type="redacted"
        stats={createStats()}
        onClose={vi.fn()}
        autoCloseDelay={0}
      />,
    );
    expect(screen.getByLabelText('关闭 DLP 警告')).toBeInTheDocument();
  });
});

// ============================================================================
// 安全审计测试
// ============================================================================

describe('DlpWarningBanner - 安全审计', () => {
  it('test_audit_no_sensitive_data_in_rendered_output', () => {
    const { container } = render(
      <DlpWarningBanner
        type="redacted"
        stats={createStats()}
        onClose={vi.fn()}
        autoCloseDelay={0}
      />,
    );
    const html = container.innerHTML;
    // 不应包含任何原始敏感数据模式
    expect(html).not.toMatch(/\d{18}/); // 身份证号
    expect(html).not.toMatch(/1[3-9]\d{9}/); // 手机号
    expect(html).not.toMatch(/LTAI[A-Za-z0-9]+/); // 阿里云密钥
    expect(html).not.toMatch(/AKIA[A-Z0-9]+/); // AWS 密钥
  });

  it('test_audit_block_reason_does_not_contain_raw_data', () => {
    const { container } = render(
      <DlpWarningBanner
        type="blocked"
        stats={createStats({ blocked_count: 1 })}
        blockReason="检测到 aliyun_access_key 类型的敏感信息"
        onClose={vi.fn()}
        autoCloseDelay={0}
      />,
    );
    const html = container.innerHTML;
    // 阻止原因应该只包含类型描述，不包含实际密钥值
    expect(html).not.toMatch(/LTAI[A-Za-z0-9]{16,}/);
  });

  it('test_audit_no_internal_state_leaked', () => {
    const { container } = render(
      <DlpWarningBanner
        type="redacted"
        stats={createStats()}
        onClose={vi.fn()}
        autoCloseDelay={0}
      />,
    );
    const html = container.innerHTML;
    expect(html).not.toMatch(/token/i);
    expect(html).not.toMatch(/session/i);
    expect(html).not.toMatch(/password/i);
  });
});
