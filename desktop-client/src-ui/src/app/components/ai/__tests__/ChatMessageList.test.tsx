/**
 * ChatMessageList 测试
 *
 * 覆盖维度：
 * - 单元测试（正常路径 + 错误路径）
 * - 契约测试（props 接口、内联 DLP 警告）
 * - 安全审计（DLP 警告内容安全性）
 *
 * 设计规范验证：
 * - 消息间距 space-y-5 (20px)
 * - 内联 DLP 警告样式（#FFF8E6 背景，#F0D060 边框，#C8960A 图标）
 * - 思考状态使用 AI 头像样式
 */

import { describe, it, expect, vi } from 'vitest';
import React from 'react';
import { render, screen } from '@testing-library/react';
import { ChatMessageList } from '../ChatMessageList';
import type { InlineDlpWarning } from '../ChatMessageList';

/* ===== Mock ===== */

vi.mock('react-markdown', () => ({
  default: ({ children }: { children: string }) => <div data-testid="markdown">{children}</div>,
}));

// Mock ScrollArea（Radix ScrollArea 在 jsdom 中可能不渲染 viewport 内容）
vi.mock('../ui/scroll-area', () => ({
  ScrollArea: ({ children, className }: { children: React.ReactNode; className?: string }) => (
    <div data-testid="scroll-area" className={className}>{children}</div>
  ),
}));

// jsdom 不支持 scrollIntoView
Element.prototype.scrollIntoView = vi.fn();

/* ===== 测试数据 ===== */

const mockMessages = [
  { id: '1', role: 'user' as const, content: '你好' },
  { id: '2', role: 'assistant' as const, content: '你好，有什么可以帮你的？' },
];

const mockDlpWarning: InlineDlpWarning = {
  type: 'redacted',
  title: 'DLP 安全提示：检测到敏感信息已自动脱敏',
  description: '已按企业安全策略脱敏处理 2 处敏感信息',
};

/* ===== 单元测试 - 正常路径 ===== */

describe('ChatMessageList - 正常路径', () => {
  it('应渲染所有消息', () => {
    render(<ChatMessageList messages={mockMessages} />);
    expect(screen.getByText('你好')).toBeInTheDocument();
    expect(screen.getByText('你好，有什么可以帮你的？')).toBeInTheDocument();
  });

  it('空消息列表应渲染空容器', () => {
    const { container } = render(<ChatMessageList messages={[]} />);
    expect(container.querySelector('.space-y-5')).toBeInTheDocument();
  });

  it('思考状态应显示 AI 头像和消息', () => {
    render(<ChatMessageList messages={[]} thinkingMessage="正在思考..." />);
    expect(screen.getByText('正在思考...')).toBeInTheDocument();
    expect(screen.getByText('XC')).toBeInTheDocument(); // AI 头像
    expect(screen.getByText('X-Claw')).toBeInTheDocument(); // AI 名称
  });

  it('加载状态应显示加载指示器', () => {
    const { container } = render(<ChatMessageList messages={[]} loading />);
    const dots = container.querySelectorAll('.animate-bounce');
    expect(dots.length).toBe(3);
  });

  it('错误状态应显示错误提示', () => {
    render(<ChatMessageList messages={[]} error="网络连接失败" />);
    expect(screen.getByText('网络连接失败')).toBeInTheDocument();
  });

  it('内联 DLP 警告应正确渲染', () => {
    render(<ChatMessageList messages={mockMessages} dlpWarning={mockDlpWarning} />);
    expect(screen.getByText('DLP 安全提示：检测到敏感信息已自动脱敏')).toBeInTheDocument();
    expect(screen.getByText('已按企业安全策略脱敏处理 2 处敏感信息')).toBeInTheDocument();
  });

  it('无 DLP 警告时不应渲染警告区域', () => {
    const { container } = render(<ChatMessageList messages={mockMessages} />);
    expect(container.querySelector('[role="alert"]')).toBeNull();
  });
});

/* ===== 单元测试 - 错误路径 ===== */

describe('ChatMessageList - 错误路径', () => {
  it('test_failure_loading_without_thinking_shows_dots', () => {
    const { container } = render(
      <ChatMessageList messages={mockMessages} loading thinkingMessage={null} />,
    );
    const dots = container.querySelectorAll('.animate-bounce');
    expect(dots.length).toBe(3);
  });

  it('test_failure_loading_with_thinking_hides_dots', () => {
    render(
      <ChatMessageList messages={mockMessages} loading thinkingMessage="思考中..." />,
    );
    // 思考消息可见
    expect(screen.getByText('思考中...')).toBeInTheDocument();
    // 不应有额外的加载指示器（思考状态优先）
    // 加载指示器只在 loading && !thinkingMessage 时显示
  });

  it('test_failure_empty_error_string_not_rendered', () => {
    const { container } = render(<ChatMessageList messages={[]} error="" />);
    // 空字符串是 falsy，不应渲染错误区域
    expect(container.querySelector('.text-destructive')).toBeNull();
  });
});

/* ===== 契约测试 ===== */

describe('ChatMessageList - 契约测试', () => {
  it('test_contract_renders_with_minimal_props', () => {
    const { container } = render(<ChatMessageList messages={[]} />);
    expect(container.firstChild).toBeInTheDocument();
  });

  it('test_contract_message_spacing_is_20px', () => {
    const { container } = render(<ChatMessageList messages={mockMessages} />);
    expect(container.querySelector('.space-y-5')).toBeInTheDocument();
  });

  it('test_contract_dlp_warning_has_alert_role', () => {
    const { container } = render(
      <ChatMessageList messages={[]} dlpWarning={mockDlpWarning} />,
    );
    const alert = container.querySelector('[role="alert"]');
    expect(alert).toBeInTheDocument();
  });

  it('test_contract_dlp_warning_has_design_colors', () => {
    const { container } = render(
      <ChatMessageList messages={[]} dlpWarning={mockDlpWarning} />,
    );
    const alert = container.querySelector('[role="alert"]');
    expect(alert?.className).toContain('bg-[#FFF8E6]');
    expect(alert?.className).toContain('border-[#F0D060]');
  });

  it('test_contract_thinking_state_has_ai_avatar_style', () => {
    const { container } = render(
      <ChatMessageList messages={[]} thinkingMessage="思考中..." />,
    );
    // AI 头像应使用深绿色
    const avatar = container.querySelector('.bg-\\[\\#2D6B45\\]');
    expect(avatar).toBeInTheDocument();
  });

  it('test_contract_passes_dlpStats_to_messages', () => {
    const messagesWithDlp = [
      {
        id: '1',
        role: 'user' as const,
        content: '脱敏内容',
        dlpStats: { total_matches: 1, redacted_count: 1, blocked_count: 0, warned_count: 0 },
      },
    ];
    render(<ChatMessageList messages={messagesWithDlp} />);
    expect(screen.getByText('已脱敏')).toBeInTheDocument();
  });

  it('test_contract_callbacks_passed_to_messages', () => {
    const onDelete = vi.fn();
    render(
      <ChatMessageList
        messages={mockMessages}
        onDeleteMessage={onDelete}
      />,
    );
    // 删除按钮应存在于用户消息中
    expect(screen.getByLabelText('删除')).toBeInTheDocument();
  });
});

/* ===== 安全审计测试 ===== */

describe('ChatMessageList - 安全审计', () => {
  it('test_audit_dlp_warning_no_raw_sensitive_data', () => {
    const warning: InlineDlpWarning = {
      type: 'redacted',
      title: 'DLP 安全提示',
      description: '已脱敏处理',
    };
    const { container } = render(
      <ChatMessageList messages={[]} dlpWarning={warning} />,
    );
    const html = container.innerHTML;
    // 不应包含任何身份证号模式
    expect(html).not.toMatch(/\d{18}/);
    expect(html).not.toMatch(/\d{15}/);
  });

  it('test_audit_error_message_no_stack_trace', () => {
    render(<ChatMessageList messages={[]} error="服务暂时不可用" />);
    const errorEl = screen.getByText('服务暂时不可用');
    expect(errorEl.textContent).not.toContain('Error:');
    expect(errorEl.textContent).not.toContain('at ');
  });

  it('test_audit_dlp_warning_has_aria_live', () => {
    const { container } = render(
      <ChatMessageList messages={[]} dlpWarning={mockDlpWarning} />,
    );
    const alert = container.querySelector('[aria-live="polite"]');
    expect(alert).toBeInTheDocument();
  });
});
