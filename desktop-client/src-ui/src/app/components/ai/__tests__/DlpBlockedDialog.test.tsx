/**
 * DlpBlockedDialog 多维度测试
 *
 * 覆盖维度：单元测试（正常路径 + 错误路径）、契约测试、安全审计测试
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { DlpBlockedDialog } from '../DlpBlockedDialog';
import type { SanitizationStats } from '../../../hooks/useDlpScan';

// ============================================================================
// 测试数据工厂
// ============================================================================

function createStats(overrides: Partial<SanitizationStats> = {}): SanitizationStats {
  return {
    total_matches: 1,
    redacted_count: 0,
    blocked_count: 1,
    warned_count: 0,
    ...overrides,
  };
}

// ============================================================================
// 单元测试 - 正常路径
// ============================================================================

describe('DlpBlockedDialog - 正常路径', () => {
  const onClose = vi.fn();
  const onEdit = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('open=true 时应渲染对话框', () => {
    render(
      <DlpBlockedDialog
        open={true}
        onClose={onClose}
        blockReason="检测到 API 密钥"
        stats={createStats()}
      />,
    );
    expect(screen.getByText('消息发送被阻止')).toBeInTheDocument();
  });

  it('应显示阻止原因', () => {
    render(
      <DlpBlockedDialog
        open={true}
        onClose={onClose}
        blockReason="检测到 aliyun_access_key"
        stats={createStats()}
      />,
    );
    expect(screen.getByText(/检测到 aliyun_access_key/)).toBeInTheDocument();
  });

  it('应显示统计 Badge', () => {
    render(
      <DlpBlockedDialog
        open={true}
        onClose={onClose}
        stats={createStats({ blocked_count: 2, redacted_count: 1 })}
      />,
    );
    expect(screen.getByText('2 处高危信息')).toBeInTheDocument();
    expect(screen.getByText('1 处已脱敏')).toBeInTheDocument();
  });

  it('点击"知道了"应触发 onClose', () => {
    render(
      <DlpBlockedDialog
        open={true}
        onClose={onClose}
        stats={createStats()}
      />,
    );
    fireEvent.click(screen.getByText('知道了'));
    expect(onClose).toHaveBeenCalled();
  });

  it('提供 onEdit 时应显示"编辑消息"按钮', () => {
    render(
      <DlpBlockedDialog
        open={true}
        onClose={onClose}
        onEdit={onEdit}
        stats={createStats()}
      />,
    );
    expect(screen.getByText('编辑消息')).toBeInTheDocument();
  });

  it('点击"编辑消息"应触发 onEdit', () => {
    render(
      <DlpBlockedDialog
        open={true}
        onClose={onClose}
        onEdit={onEdit}
        stats={createStats()}
      />,
    );
    fireEvent.click(screen.getByText('编辑消息'));
    expect(onEdit).toHaveBeenCalledTimes(1);
  });

  it('open=false 时不应渲染对话框内容', () => {
    render(
      <DlpBlockedDialog
        open={false}
        onClose={onClose}
        stats={createStats()}
      />,
    );
    expect(screen.queryByText('消息发送被阻止')).not.toBeInTheDocument();
  });
});

// ============================================================================
// 单元测试 - 错误路径
// ============================================================================

describe('DlpBlockedDialog - 错误路径', () => {
  it('test_failure_no_block_reason_shows_default_message', () => {
    render(
      <DlpBlockedDialog
        open={true}
        onClose={vi.fn()}
        stats={createStats()}
      />,
    );
    expect(
      screen.getByText(/高危敏感信息/),
    ).toBeInTheDocument();
  });

  it('test_failure_no_stats_still_renders', () => {
    expect(() => {
      render(
        <DlpBlockedDialog
          open={true}
          onClose={vi.fn()}
        />,
      );
    }).not.toThrow();
    expect(screen.getByText('消息发送被阻止')).toBeInTheDocument();
  });

  it('test_failure_no_onEdit_hides_edit_button', () => {
    render(
      <DlpBlockedDialog
        open={true}
        onClose={vi.fn()}
        stats={createStats()}
      />,
    );
    expect(screen.queryByText('编辑消息')).not.toBeInTheDocument();
  });
});

// ============================================================================
// 契约测试
// ============================================================================

describe('DlpBlockedDialog - 契约测试', () => {
  it('test_contract_renders_with_minimal_props', () => {
    expect(() => {
      render(<DlpBlockedDialog open={false} onClose={vi.fn()} />);
    }).not.toThrow();
  });

  it('test_contract_dialog_has_correct_structure', () => {
    render(
      <DlpBlockedDialog
        open={true}
        onClose={vi.fn()}
        blockReason="test"
        stats={createStats()}
      />,
    );
    // 应有标题、描述、操作按钮
    expect(screen.getByText('消息发送被阻止')).toBeInTheDocument();
    expect(screen.getByText(/阻止原因/)).toBeInTheDocument();
    expect(screen.getByText('知道了')).toBeInTheDocument();
  });

  it('test_contract_shows_help_text', () => {
    render(
      <DlpBlockedDialog
        open={true}
        onClose={vi.fn()}
        stats={createStats()}
      />,
    );
    expect(
      screen.getByText(/请移除敏感信息后重新发送/),
    ).toBeInTheDocument();
  });
});

// ============================================================================
// 安全审计测试
// ============================================================================

describe('DlpBlockedDialog - 安全审计', () => {
  it('test_audit_no_sensitive_data_patterns_in_output', () => {
    render(
      <DlpBlockedDialog
        open={true}
        onClose={vi.fn()}
        blockReason="检测到敏感信息类型"
        stats={createStats()}
      />,
    );
    // AlertDialog 使用 portal 渲染到 document.body
    const html = document.body.innerHTML;
    expect(html).not.toMatch(/\d{18}/);
    expect(html).not.toMatch(/1[3-9]\d{9}/);
    expect(html).not.toMatch(/LTAI[A-Za-z0-9]+/);
    expect(html).not.toMatch(/AKIA[A-Z0-9]+/);
    expect(html).not.toMatch(/-----BEGIN/);
  });

  it('test_audit_block_reason_is_type_only', () => {
    render(
      <DlpBlockedDialog
        open={true}
        onClose={vi.fn()}
        blockReason="aliyun_access_key"
        stats={createStats()}
      />,
    );
    // AlertDialog 使用 portal 渲染，需要从 document.body 查找
    const html = document.body.innerHTML;
    // 应只包含类型名称，不包含实际密钥
    expect(html).toContain('aliyun_access_key');
    expect(html).not.toMatch(/LTAI[A-Za-z0-9]{16,}/);
  });

  it('test_audit_no_internal_error_details', () => {
    render(
      <DlpBlockedDialog
        open={true}
        onClose={vi.fn()}
        blockReason="检测到敏感信息"
        stats={createStats()}
      />,
    );
    const html = document.body.innerHTML;
    expect(html).not.toMatch(/stack/i);
    expect(html).not.toMatch(/trace/i);
    expect(html).not.toMatch(/internal/i);
  });
});
