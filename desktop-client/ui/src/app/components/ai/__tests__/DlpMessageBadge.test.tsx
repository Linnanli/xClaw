/**
 * DlpMessageBadge 多维度测试
 *
 * 覆盖维度：单元测试（正常路径 + 错误路径）、契约测试、安全审计测试
 */

import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { DlpMessageBadge } from '../DlpMessageBadge';
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

describe('DlpMessageBadge - 正常路径', () => {
  it('有脱敏数据时应渲染 Badge', () => {
    render(<DlpMessageBadge stats={createStats()} />);
    expect(screen.getByText('已脱敏')).toBeInTheDocument();
  });

  it('有警告数据时应渲染 Badge', () => {
    render(
      <DlpMessageBadge stats={createStats({ redacted_count: 0, warned_count: 1 })} />,
    );
    expect(screen.getByText('已脱敏')).toBeInTheDocument();
  });
});

// ============================================================================
// 单元测试 - 错误路径
// ============================================================================

describe('DlpMessageBadge - 错误路径', () => {
  it('test_failure_zero_counts_returns_null', () => {
    const { container } = render(
      <DlpMessageBadge stats={createStats({ redacted_count: 0, warned_count: 0 })} />,
    );
    expect(container.innerHTML).toBe('');
  });

  it('test_failure_all_zero_stats_returns_null', () => {
    const { container } = render(
      <DlpMessageBadge
        stats={createStats({
          total_matches: 0,
          redacted_count: 0,
          blocked_count: 0,
          warned_count: 0,
        })}
      />,
    );
    expect(container.innerHTML).toBe('');
  });
});

// ============================================================================
// 契约测试
// ============================================================================

describe('DlpMessageBadge - 契约测试', () => {
  it('test_contract_renders_with_minimal_stats', () => {
    expect(() => {
      render(<DlpMessageBadge stats={createStats()} />);
    }).not.toThrow();
  });

  it('test_contract_accepts_custom_className', () => {
    const { container } = render(
      <DlpMessageBadge stats={createStats()} className="my-custom" />,
    );
    expect(container.querySelector('.my-custom')).toBeInTheDocument();
  });

  it('test_contract_badge_is_not_interactive', () => {
    render(<DlpMessageBadge stats={createStats()} />);
    // Badge 应该存在且包含"已脱敏"文本
    const badge = screen.getByText('已脱敏');
    expect(badge).toBeInTheDocument();
    // Badge 不应是 button 角色
    expect(badge.closest('button')).toBeNull();
  });
});

// ============================================================================
// 安全审计测试
// ============================================================================

describe('DlpMessageBadge - 安全审计', () => {
  it('test_audit_no_sensitive_data_in_badge', () => {
    const { container } = render(
      <DlpMessageBadge stats={createStats({ total_matches: 5, redacted_count: 3 })} />,
    );
    const html = container.innerHTML;
    // 只应包含数字统计，不应包含任何敏感数据模式
    expect(html).not.toMatch(/\d{11}/); // 手机号
    expect(html).not.toMatch(/\d{18}/); // 身份证号
    expect(html).not.toMatch(/LTAI/);
    expect(html).not.toMatch(/AKIA/);
  });

  it('test_audit_only_shows_counts_not_content', () => {
    const { container } = render(
      <DlpMessageBadge stats={createStats()} />,
    );
    const html = container.innerHTML;
    // 应只包含"已脱敏"文本和数字
    expect(html).toContain('已脱敏');
    expect(html).not.toMatch(/password/i);
    expect(html).not.toMatch(/secret/i);
    expect(html).not.toMatch(/token/i);
  });
});
