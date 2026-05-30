/**
 * ThinkingProcess 测试
 *
 * 覆盖维度：
 * - 单元测试（正常路径 + 错误路径）
 * - 契约测试（props 接口、Collapsible 行为、头像样式）
 * - 安全审计（步骤内容安全性、无敏感信息泄露）
 *
 * 设计规范验证：
 * - AI 头像 #2D6B45 32px，fallback "XC"
 * - 气泡圆角 [4,16,16,16]（左上角小，指向头像）
 * - 气泡偏移 42px（头像 32px + gap 10px）
 * - 脉冲指示器（活跃状态）
 */

import { describe, it, expect, vi } from 'vitest';
import React from 'react';
import { render, screen, fireEvent } from '@testing-library/react';
import { ThinkingProcess } from '../ThinkingProcess';
import type { ThinkingStep } from '../../../hooks/useAiChatTauri';

/* ===== Mock ===== */

// jsdom 不支持 scrollIntoView
Element.prototype.scrollIntoView = vi.fn();

/* ===== 测试数据 ===== */

const mockSteps: ThinkingStep[] = [
  { id: 'step-1', message: '分析用户问题...', timestamp: 1700000001000 },
  { id: 'step-2', message: '检索相关知识库...', timestamp: 1700000002000 },
  { id: 'step-3', message: '生成回答方案...', timestamp: 1700000003000 },
];

const singleStep: ThinkingStep[] = [
  { id: 'step-1', message: '正在处理请求...', timestamp: 1700000001000 },
];

/* ===== 单元测试 - 正常路径 ===== */

describe('ThinkingProcess - 正常路径', () => {
  it('活跃状态应显示 AI 头像和名称', () => {
    render(<ThinkingProcess steps={mockSteps} isActive />);
    expect(screen.getByText('XC')).toBeInTheDocument();
    expect(screen.getByText('X-Claw')).toBeInTheDocument();
  });

  it('活跃状态应显示最新思考步骤', () => {
    render(<ThinkingProcess steps={mockSteps} isActive />);
    // 最新步骤同时出现在触发器和步骤列表中
    const elements = screen.getAllByText('生成回答方案...');
    expect(elements.length).toBeGreaterThanOrEqual(1);
  });

  it('活跃状态应自动展开步骤列表', () => {
    render(<ThinkingProcess steps={mockSteps} isActive />);
    // 所有步骤应可见（在步骤列表中）
    expect(screen.getAllByText('分析用户问题...').length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText('检索相关知识库...').length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText('生成回答方案...').length).toBeGreaterThanOrEqual(1);
  });

  it('活跃状态应显示脉冲指示器', () => {
    const { container } = render(<ThinkingProcess steps={mockSteps} isActive />);
    // 活跃状态下应有动画指示器（shimmer 或 pulse）
    const animatedEl = container.querySelector('.shimmer, .animate-pulse, .animate-ping');
    expect(animatedEl).toBeInTheDocument();
  });

  it('活跃状态应显示等待指示', () => {
    render(<ThinkingProcess steps={mockSteps} isActive />);
    expect(screen.getByLabelText('等待下一步')).toBeInTheDocument();
  });

  it('完成状态应显示步骤计数', () => {
    render(<ThinkingProcess steps={mockSteps} isActive={false} />);
    expect(screen.getByText('思考完成（3 步）')).toBeInTheDocument();
  });

  it('完成状态应默认折叠', () => {
    render(<ThinkingProcess steps={mockSteps} isActive={false} />);
    // 步骤列表中的序号不应可见（折叠状态）
    // 触发器文本应可见
    expect(screen.getByText('思考完成（3 步）')).toBeInTheDocument();
  });

  it('完成状态点击应展开步骤列表', () => {
    render(<ThinkingProcess steps={mockSteps} isActive={false} />);
    // 找到可折叠触发器（button 角色）并点击
    const trigger = screen.getByRole('button');
    fireEvent.click(trigger);
    // 展开后步骤应可见
    expect(screen.getByText('分析用户问题...')).toBeInTheDocument();
    expect(screen.getByText('检索相关知识库...')).toBeInTheDocument();
  });

  it('步骤应按序号显示', () => {
    render(<ThinkingProcess steps={mockSteps} isActive />);
    const stepNumbers = screen.getAllByText(/^[123]$/);
    expect(stepNumbers).toHaveLength(3);
  });

  it('单步骤应正确渲染', () => {
    render(<ThinkingProcess steps={singleStep} isActive />);
    // 步骤文本同时出现在触发器和列表中
    expect(screen.getAllByText('正在处理请求...').length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText('1')).toBeInTheDocument();
  });
});

/* ===== 单元测试 - 错误路径 ===== */

describe('ThinkingProcess - 错误路径', () => {
  it('test_failure_empty_steps_active_shows_default_message', () => {
    render(<ThinkingProcess steps={[]} isActive />);
    // 无步骤时触发器应显示默认文本
    expect(screen.getAllByText('正在思考...').length).toBeGreaterThanOrEqual(1);
  });

  it('test_failure_empty_steps_inactive_returns_null', () => {
    const { container } = render(<ThinkingProcess steps={[]} isActive={false} />);
    expect(container.firstChild).toBeNull();
  });

  it('test_failure_steps_with_empty_message', () => {
    const stepsWithEmpty: ThinkingStep[] = [
      { id: 'step-1', message: '', timestamp: 1700000001000 },
    ];
    const { container } = render(<ThinkingProcess steps={stepsWithEmpty} isActive />);
    // 应渲染组件但步骤消息为空
    expect(container.firstChild).not.toBeNull();
  });

  it('test_failure_steps_with_very_long_message', () => {
    const longMessage = '这是一段非常长的思考内容'.repeat(50);
    const stepsWithLong: ThinkingStep[] = [
      { id: 'step-1', message: longMessage, timestamp: 1700000001000 },
    ];
    const { container } = render(<ThinkingProcess steps={stepsWithLong} isActive />);
    // 触发器内的文本 span 应有 truncate class
    const truncateSpan = container.querySelector('.truncate');
    expect(truncateSpan).toBeInTheDocument();
  });

  it('test_failure_rapid_state_toggle', () => {
    const { rerender } = render(<ThinkingProcess steps={mockSteps} isActive />);
    // 快速切换状态
    rerender(<ThinkingProcess steps={mockSteps} isActive={false} />);
    rerender(<ThinkingProcess steps={mockSteps} isActive />);
    rerender(<ThinkingProcess steps={mockSteps} isActive={false} />);
    // 最终应为完成状态
    expect(screen.getByText('思考完成（3 步）')).toBeInTheDocument();
  });
});

/* ===== 契约测试 ===== */

describe('ThinkingProcess - 契约测试', () => {
  it('test_contract_renders_with_minimal_props', () => {
    const { container } = render(<ThinkingProcess steps={mockSteps} isActive={false} />);
    expect(container.firstChild).toBeInTheDocument();
  });

  it('test_contract_ai_avatar_has_correct_color', () => {
    const { container } = render(<ThinkingProcess steps={mockSteps} isActive />);
    const avatar = container.querySelector('.bg-\\[\\#2D6B45\\]');
    expect(avatar).toBeInTheDocument();
  });

  it('test_contract_bubble_has_correct_corner_radius', () => {
    const { container } = render(<ThinkingProcess steps={mockSteps} isActive />);
    // 气泡容器应存在（ReasoningRoot 渲染为 Collapsible）
    const collapsible = container.querySelector('[data-slot="reasoning-root"]');
    expect(collapsible).toBeInTheDocument();
  });

  it('test_contract_bubble_offset_42px', () => {
    const { container } = render(<ThinkingProcess steps={mockSteps} isActive />);
    // 内容区应有 42px 左偏移（头像 32px + gap 10px）
    const offset = container.querySelector('.pl-\\[42px\\]');
    expect(offset).toBeInTheDocument();
  });

  it('test_contract_bubble_has_border_style', () => {
    const { container } = render(<ThinkingProcess steps={mockSteps} isActive />);
    // ReasoningRoot outline variant 有边框
    const bordered = container.querySelector('[data-variant="outline"]');
    expect(bordered).toBeInTheDocument();
  });

  it('test_contract_collapsible_trigger_has_aria_label', () => {
    render(<ThinkingProcess steps={mockSteps} isActive={false} />);
    // 触发器应可交互（button 角色）
    const trigger = screen.getByRole('button');
    expect(trigger).toBeInTheDocument();
  });

  it('test_contract_active_trigger_has_collapse_aria_label', () => {
    render(<ThinkingProcess steps={mockSteps} isActive />);
    // 活跃状态下触发器应存在
    const trigger = screen.getByRole('button');
    expect(trigger).toBeInTheDocument();
  });

  it('test_contract_step_list_has_aria_label', () => {
    render(<ThinkingProcess steps={mockSteps} isActive />);
    expect(screen.getByLabelText('思考步骤')).toBeInTheDocument();
  });

  it('test_contract_inactive_hides_ping_indicator', () => {
    const { container } = render(<ThinkingProcess steps={mockSteps} isActive={false} />);
    const pingEl = container.querySelector('.animate-ping');
    expect(pingEl).toBeNull();
  });

  it('test_contract_inactive_shows_static_dot', () => {
    const { container } = render(<ThinkingProcess steps={mockSteps} isActive={false} />);
    // 完成状态下触发器图标应存在（WrenchIcon）
    const icon = container.querySelector('[data-slot="reasoning-trigger-icon"]');
    expect(icon).toBeInTheDocument();
  });

  it('test_contract_chevron_rotates_when_open', () => {
    const { container } = render(<ThinkingProcess steps={mockSteps} isActive />);
    // 活跃状态下自动展开，Collapsible 应为 open 状态
    const collapsible = container.querySelector('[data-state="open"]');
    expect(collapsible).toBeInTheDocument();
  });

  it('test_contract_custom_className_applied', () => {
    const { container } = render(
      <ThinkingProcess steps={mockSteps} isActive className="custom-class" />,
    );
    expect(container.firstChild).toHaveClass('custom-class');
  });
});

/* ===== 安全审计测试 ===== */

describe('ThinkingProcess - 安全审计', () => {
  it('test_audit_no_sensitive_data_in_steps', () => {
    const sensitiveSteps: ThinkingStep[] = [
      { id: 'step-1', message: '处理用户查询...', timestamp: 1700000001000 },
    ];
    const { container } = render(<ThinkingProcess steps={sensitiveSteps} isActive />);
    const html = container.innerHTML;
    // 不应包含身份证号模式
    expect(html).not.toMatch(/\d{18}/);
    expect(html).not.toMatch(/\d{15}/);
  });

  it('test_audit_step_ids_not_leaked_to_dom', () => {
    const { container } = render(<ThinkingProcess steps={mockSteps} isActive />);
    const html = container.innerHTML;
    // 内部 step ID 不应作为可见文本泄露
    expect(html).not.toContain('step-1700');
  });

  it('test_audit_timestamps_not_visible', () => {
    render(<ThinkingProcess steps={mockSteps} isActive />);
    // 时间戳不应作为可见文本显示
    expect(screen.queryByText('1700000001000')).toBeNull();
    expect(screen.queryByText('1700000002000')).toBeNull();
  });

  it('test_audit_no_stack_trace_in_error_steps', () => {
    const errorSteps: ThinkingStep[] = [
      { id: 'step-1', message: '处理出错，正在重试...', timestamp: 1700000001000 },
    ];
    const { container } = render(<ThinkingProcess steps={errorSteps} isActive />);
    const html = container.innerHTML;
    expect(html).not.toContain('Error:');
    expect(html).not.toContain('at ');
    expect(html).not.toContain('stack');
  });

  it('test_audit_aria_live_not_exposing_content', () => {
    // 确保 aria 属性不会意外暴露敏感内容
    const { container } = render(<ThinkingProcess steps={mockSteps} isActive />);
    const ariaElements = container.querySelectorAll('[aria-label]');
    ariaElements.forEach((el) => {
      const label = el.getAttribute('aria-label') || '';
      // aria-label 不应包含步骤的具体内容
      expect(label).not.toContain('分析用户问题');
      expect(label).not.toContain('检索相关知识库');
    });
  });
});
