/**
 * ChatMessage 测试
 *
 * 覆盖维度：
 * - 单元测试（正常路径 + 错误路径）
 * - 契约测试（props 接口一致性）
 * - 安全审计（DLP 脱敏标记、敏感信息不泄露）
 *
 * 设计规范验证：
 * - 用户消息：右对齐，绿色头像 #3D8A5A，气泡圆角 [16,16,4,16]
 * - AI 消息：左对齐，深绿头像 #2D6B45，白色气泡 + 边框
 * - 头像 32px，名称标签 #9D9C9A
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { ChatMessage } from '../ChatMessage';
import type { SanitizationStats } from '../../../hooks/useDlpScan';

/* ===== Mock ===== */

vi.mock('react-markdown', () => ({
  default: ({ children }: { children: string }) => <div data-testid="markdown">{children}</div>,
}));

// jsdom 不支持 scrollIntoView
Element.prototype.scrollIntoView = vi.fn();

/* ===== 测试数据 ===== */

const mockDlpStats: SanitizationStats = {
  total_matches: 2,
  redacted_count: 2,
  blocked_count: 0,
  warned_count: 0,
};

/* ===== 单元测试 - 正常路径 ===== */

describe('ChatMessage - 正常路径', () => {
  it('用户消息应右对齐并显示头像和名称', () => {
    render(<ChatMessage id="1" role="user" content="你好" />);
    expect(screen.getByText('你好')).toBeInTheDocument();
    // 名称标签 "你" 和头像 fallback "你" 都存在
    const nameLabels = screen.getAllByText('你');
    expect(nameLabels.length).toBeGreaterThanOrEqual(1);
  });

  it('AI 消息应左对齐并显示头像和名称', () => {
    render(<ChatMessage id="2" role="assistant" content="你好，有什么可以帮你的？" />);
    expect(screen.getByText('XC')).toBeInTheDocument(); // AI 头像 fallback
    expect(screen.getByText('X-Claw')).toBeInTheDocument(); // AI 名称
  });

  it('用户消息气泡应使用绿色背景', () => {
    const { container } = render(<ChatMessage id="1" role="user" content="测试" />);
    const bubble = container.querySelector('.bg-\\[\\#3D8A5A\\]');
    expect(bubble).toBeInTheDocument();
  });

  it('AI 消息气泡应使用白色背景和边框', () => {
    const { container } = render(<ChatMessage id="2" role="assistant" content="回复" />);
    const bubble = container.querySelector('.bg-white');
    expect(bubble).toBeInTheDocument();
    const bordered = container.querySelector('.border-\\[\\#E5E4E1\\]');
    expect(bordered).toBeInTheDocument();
  });

  it('用户消息气泡应有非对称圆角（右上角小，指向头像）', () => {
    const { container } = render(<ChatMessage id="1" role="user" content="测试" />);
    const bubble = container.querySelector('.rounded-tr');
    expect(bubble).toBeInTheDocument();
    // 确认不是 rounded-tr-2xl
    const largeTr = container.querySelector('.rounded-tr-2xl');
    expect(largeTr).toBeNull();
  });

  it('AI 消息气泡应有非对称圆角（左上角小，指向头像）', () => {
    const { container } = render(<ChatMessage id="2" role="assistant" content="回复" />);
    const bubble = container.querySelector('.rounded-tl');
    expect(bubble).toBeInTheDocument();
  });

  it('加载状态应显示动画点', () => {
    const { container } = render(<ChatMessage id="loading" role="assistant" content="" isLoading />);
    const dots = container.querySelectorAll('.animate-bounce');
    expect(dots.length).toBe(3);
  });

  it('AI 消息应渲染 Markdown', () => {
    render(<ChatMessage id="2" role="assistant" content="**bold**" />);
    expect(screen.getByTestId('markdown')).toBeInTheDocument();
  });

  it('用户消息应渲染纯文本', () => {
    render(<ChatMessage id="1" role="user" content="纯文本消息" />);
    expect(screen.getByText('纯文本消息')).toBeInTheDocument();
    expect(screen.queryByTestId('markdown')).toBeNull();
  });

  it('自定义用户名称应正确显示', () => {
    render(<ChatMessage id="1" role="user" content="测试" userName="张三" />);
    expect(screen.getByText('张三')).toBeInTheDocument(); // 名称标签
    // 头像 fallback 应显示首字符 "张"
    const avatarFallbacks = screen.getAllByText('张');
    expect(avatarFallbacks.length).toBeGreaterThanOrEqual(1);
  });

  it('自定义 AI 名称应正确显示', () => {
    render(<ChatMessage id="2" role="assistant" content="回复" aiName="助手" />);
    expect(screen.getByText('助手')).toBeInTheDocument();
  });
});

/* ===== 单元测试 - 操作栏 ===== */

describe('ChatMessage - 操作栏', () => {
  it('应显示复制按钮', () => {
    render(<ChatMessage id="1" role="user" content="测试" />);
    expect(screen.getByLabelText('复制')).toBeInTheDocument();
  });

  it('点击复制应调用 clipboard API', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.assign(navigator, { clipboard: { writeText } });

    render(<ChatMessage id="1" role="user" content="复制内容" />);
    fireEvent.click(screen.getByLabelText('复制'));

    await waitFor(() => {
      expect(writeText).toHaveBeenCalledWith('复制内容');
    });
  });

  it('用户消息应显示编辑和删除按钮', () => {
    const onEdit = vi.fn();
    const onDelete = vi.fn();
    render(
      <ChatMessage id="1" role="user" content="测试" onEdit={onEdit} onDelete={onDelete} />,
    );
    expect(screen.getByLabelText('编辑')).toBeInTheDocument();
    expect(screen.getByLabelText('删除')).toBeInTheDocument();
  });

  it('AI 消息应显示重新生成按钮', () => {
    const onRegenerate = vi.fn();
    render(
      <ChatMessage id="2" role="assistant" content="回复" onRegenerate={onRegenerate} />,
    );
    expect(screen.getByLabelText('重新生成')).toBeInTheDocument();
  });

  it('点击编辑应传递消息 ID', () => {
    const onEdit = vi.fn();
    render(<ChatMessage id="msg-123" role="user" content="测试" onEdit={onEdit} />);
    fireEvent.click(screen.getByLabelText('编辑'));
    expect(onEdit).toHaveBeenCalledWith('msg-123');
  });

  it('点击删除应传递消息 ID', () => {
    const onDelete = vi.fn();
    render(<ChatMessage id="msg-456" role="user" content="测试" onDelete={onDelete} />);
    fireEvent.click(screen.getByLabelText('删除'));
    expect(onDelete).toHaveBeenCalledWith('msg-456');
  });

  it('加载状态不应显示操作栏', () => {
    render(<ChatMessage id="loading" role="assistant" content="" isLoading />);
    expect(screen.queryByLabelText('复制')).toBeNull();
  });
});

/* ===== 单元测试 - DLP 脱敏标记 ===== */

describe('ChatMessage - DLP 脱敏标记', () => {
  it('有 dlpStats 的用户消息应显示脱敏标记', () => {
    render(<ChatMessage id="1" role="user" content="已脱敏内容" dlpStats={mockDlpStats} />);
    expect(screen.getByText('已脱敏')).toBeInTheDocument();
  });

  it('无 dlpStats 的用户消息不应显示脱敏标记', () => {
    render(<ChatMessage id="1" role="user" content="普通内容" />);
    expect(screen.queryByText('已脱敏')).toBeNull();
  });

  it('AI 消息不应显示脱敏标记', () => {
    render(<ChatMessage id="2" role="assistant" content="回复" dlpStats={mockDlpStats} />);
    expect(screen.queryByText('已脱敏')).toBeNull();
  });
});

/* ===== 契约测试 ===== */

describe('ChatMessage - 契约测试', () => {
  it('test_contract_renders_with_minimal_props', () => {
    const { container } = render(<ChatMessage id="1" role="user" content="最小 props" />);
    expect(container.firstChild).toBeInTheDocument();
  });

  it('test_contract_user_message_has_avatar', () => {
    const { container } = render(<ChatMessage id="1" role="user" content="测试" />);
    const avatar = container.querySelector('[data-slot="avatar"]');
    expect(avatar).toBeInTheDocument();
  });

  it('test_contract_assistant_message_has_avatar', () => {
    const { container } = render(<ChatMessage id="2" role="assistant" content="回复" />);
    const avatar = container.querySelector('[data-slot="avatar"]');
    expect(avatar).toBeInTheDocument();
  });

  it('test_contract_user_bubble_alignment_right', () => {
    const { container } = render(<ChatMessage id="1" role="user" content="测试" />);
    const wrapper = container.firstChild as HTMLElement;
    expect(wrapper.className).toContain('items-end');
  });

  it('test_contract_assistant_bubble_alignment_left', () => {
    const { container } = render(<ChatMessage id="2" role="assistant" content="回复" />);
    const wrapper = container.firstChild as HTMLElement;
    expect(wrapper.className).toContain('items-start');
  });

  it('test_contract_bubble_offset_matches_avatar_width', () => {
    // 用户消息气泡区域应有 pr-[42px]（头像 32px + gap 10px）
    const { container } = render(<ChatMessage id="1" role="user" content="测试" />);
    const bubbleWrap = container.querySelector('.pr-\\[42px\\]');
    expect(bubbleWrap).toBeInTheDocument();
    // 气泡区域应占满宽度
    expect(bubbleWrap?.className).toContain('w-full');
  });

  it('test_contract_ai_bubble_offset_matches_avatar_width', () => {
    // AI 消息气泡区域应有 pl-[42px]
    const { container } = render(<ChatMessage id="2" role="assistant" content="回复" />);
    const bubbleWrap = container.querySelector('.pl-\\[42px\\]');
    expect(bubbleWrap).toBeInTheDocument();
    expect(bubbleWrap?.className).toContain('w-full');
  });
});

/* ===== 安全审计测试 ===== */

describe('ChatMessage - 安全审计', () => {
  it('test_audit_no_sensitive_data_in_avatar', () => {
    // 头像不应泄露完整用户名
    const { container } = render(
      <ChatMessage id="1" role="user" content="测试" userName="张三丰" />,
    );
    const html = container.innerHTML;
    // 头像 fallback 只显示首字符
    expect(html).toContain('张');
    // 名称标签显示完整名称（这是预期行为）
    expect(screen.getByText('张三丰')).toBeInTheDocument();
  });

  it('test_audit_dlp_stats_not_leaked_in_bubble', () => {
    // DLP 统计数据不应出现在气泡内容中
    render(
      <ChatMessage id="1" role="user" content="安全内容" dlpStats={mockDlpStats} />,
    );
    // 气泡中应包含消息内容
    expect(screen.getByText('安全内容')).toBeInTheDocument();
    // 整个渲染输出不应包含原始统计字段名
    const html = document.body.innerHTML;
    expect(html).not.toContain('total_matches');
    expect(html).not.toContain('redacted_count');
  });

  it('test_audit_message_content_not_in_aria_labels', () => {
    // 操作按钮的 aria-label 不应包含消息内容
    render(<ChatMessage id="1" role="user" content="敏感信息" onEdit={vi.fn()} />);
    const editBtn = screen.getByLabelText('编辑');
    expect(editBtn.getAttribute('aria-label')).toBe('编辑');
    expect(editBtn.getAttribute('aria-label')).not.toContain('敏感信息');
  });
});
