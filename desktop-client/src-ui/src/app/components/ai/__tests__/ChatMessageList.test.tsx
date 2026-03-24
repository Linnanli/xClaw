/**
 * ChatMessageList 测试
 *
 * 覆盖维度：
 * - 单元测试（正常路径 + 错误路径）
 * - 契约测试（props 接口、内联 DLP 警告）
 * - 安全审计（DLP 警告内容安全性）
 *
 * 重构说明：
 * thinkingSteps 已嵌入 assistant 消息自身（参考 Vercel AI SDK message.parts 模式），
 * ChatMessageList 不再接收独立的 thinkingSteps prop。
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

vi.mock('../ui/scroll-area', () => ({
  ScrollArea: ({ children, className }: { children: React.ReactNode; className?: string }) => (
    <div data-testid="scroll-area" className={className}>{children}</div>
  ),
}));

vi.mock('../ThinkingProcess', () => ({
  ThinkingProcess: ({ steps, isActive, hideAvatar }: { steps: unknown[]; isActive: boolean; hideAvatar?: boolean }) => (
    <div data-testid="thinking-process" data-active={isActive} data-steps={steps.length} data-hide-avatar={hideAvatar ?? false}>
      {isActive ? '思考中...' : `思考完成（${steps.length} 步）`}
    </div>
  ),
}));

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

  it('thinkingMessage 应渲染实时思考指示器', () => {
    render(<ChatMessageList messages={[]} thinkingMessage="正在思考..." />);
    const tp = screen.getByTestId('thinking-process');
    expect(tp).toBeInTheDocument();
    expect(tp.getAttribute('data-active')).toBe('true');
  });

  it('assistant 消息自带 thinkingSteps 应渲染折叠的思考过程', () => {
    const messagesWithThinking = [
      { id: '1', role: 'user' as const, content: '你好' },
      {
        id: '2',
        role: 'assistant' as const,
        content: '回复',
        thinkingSteps: [
          { id: 'step-1', message: 'Processing...', timestamp: Date.now() },
          { id: 'step-2', message: 'Calling LLM...', timestamp: Date.now() },
        ],
      },
    ];
    render(<ChatMessageList messages={messagesWithThinking} />);
    const tp = screen.getByTestId('thinking-process');
    expect(tp).toBeInTheDocument();
    expect(tp.getAttribute('data-active')).toBe('false');
    expect(tp.getAttribute('data-steps')).toBe('2');
    expect(tp.getAttribute('data-hide-avatar')).toBe('true');
  });

  it('思考过程应在 AI 回复之前渲染（顺序正确）', () => {
    const messagesWithThinking = [
      { id: '1', role: 'user' as const, content: '你好' },
      {
        id: '2',
        role: 'assistant' as const,
        content: 'AI 回复',
        thinkingSteps: [{ id: 'step-1', message: 'Thinking...', timestamp: Date.now() }],
      },
    ];
    const { container } = render(<ChatMessageList messages={messagesWithThinking} />);
    const html = container.innerHTML;
    const thinkingIdx = html.indexOf('thinking-process');
    const replyIdx = html.indexOf('AI 回复');
    expect(thinkingIdx).toBeLessThan(replyIdx);
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

  it('test_failure_loading_with_thinking_shows_thinking_process', () => {
    render(
      <ChatMessageList messages={mockMessages} loading thinkingMessage="思考中..." />,
    );
    expect(screen.getByTestId('thinking-process')).toBeInTheDocument();
  });

  it('test_failure_empty_error_string_not_rendered', () => {
    const { container } = render(<ChatMessageList messages={[]} error="" />);
    expect(container.querySelector('.text-destructive')).toBeNull();
  });

  it('test_failure_assistant_without_thinkingSteps_no_thinking_process', () => {
    const msgs = [
      { id: '1', role: 'user' as const, content: '你好' },
      { id: '2', role: 'assistant' as const, content: '回复' },
    ];
    render(<ChatMessageList messages={msgs} />);
    expect(screen.queryByTestId('thinking-process')).toBeNull();
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
    expect(container.querySelector('[role="alert"]')).toBeInTheDocument();
  });

  it('test_contract_dlp_warning_has_design_colors', () => {
    const { container } = render(
      <ChatMessageList messages={[]} dlpWarning={mockDlpWarning} />,
    );
    const alert = container.querySelector('[role="alert"]');
    expect(alert?.className).toContain('bg-[#FFF8E6]');
    expect(alert?.className).toContain('border-[#F0D060]');
  });

  it('test_contract_thinking_state_renders_thinking_process', () => {
    render(<ChatMessageList messages={[]} thinkingMessage="思考中..." />);
    const tp = screen.getByTestId('thinking-process');
    expect(tp).toBeInTheDocument();
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
    expect(screen.getByLabelText('删除')).toBeInTheDocument();
  });

  it('test_contract_multiple_assistant_messages_each_with_own_thinking', () => {
    const msgs = [
      { id: '1', role: 'user' as const, content: '问题1' },
      {
        id: '2',
        role: 'assistant' as const,
        content: '回复1',
        thinkingSteps: [{ id: 's1', message: 'Step A', timestamp: 1 }],
      },
      { id: '3', role: 'user' as const, content: '问题2' },
      {
        id: '4',
        role: 'assistant' as const,
        content: '回复2',
        thinkingSteps: [{ id: 's2', message: 'Step B', timestamp: 2 }],
      },
    ];
    render(<ChatMessageList messages={msgs} />);
    const tps = screen.getAllByTestId('thinking-process');
    expect(tps).toHaveLength(2);
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
    expect(container.querySelector('[aria-live="polite"]')).toBeInTheDocument();
  });
});
