/**
 * PasswordSetup 单元测试
 *
 * 覆盖维度：正常路径、错误路径、契约测试、安全审计
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { PasswordSetup } from '../PasswordSetup';

const mockNavigate = vi.fn();

vi.mock('react-router', () => ({
  useNavigate: () => mockNavigate,
}));

vi.mock('../../../utils/tauri', () => ({
  authApi: {
    setupMasterPassword: vi.fn().mockResolvedValue({ success: true }),
  },
}));

describe('PasswordSetup', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // ===== 正常路径测试 =====

  it('应该渲染左侧品牌面板', () => {
    render(<PasswordSetup />);
    expect(screen.getByText('X-Claw')).toBeInTheDocument();
    expect(screen.getByText('政企级 AI 智能助手')).toBeInTheDocument();
  });

  it('应该渲染创建密码表单', () => {
    render(<PasswordSetup />);
    expect(screen.getByText('创建主密码')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('至少12个字符')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('再次输入密码')).toBeInTheDocument();
  });

  it('应该显示密码要求列表', () => {
    render(<PasswordSetup />);
    expect(screen.getByText('至少12个字符')).toBeInTheDocument();
    expect(screen.getByText('包含大写字母')).toBeInTheDocument();
    expect(screen.getByText('包含小写字母')).toBeInTheDocument();
    expect(screen.getByText('包含数字')).toBeInTheDocument();
  });

  it('输入密码时应显示强度指示器', () => {
    render(<PasswordSetup />);
    const passwordInput = screen.getByPlaceholderText('至少12个字符');
    fireEvent.change(passwordInput, { target: { value: 'short' } });
    expect(screen.getByText('弱')).toBeInTheDocument();
  });

  it('强密码应显示"强"', () => {
    render(<PasswordSetup />);
    const passwordInput = screen.getByPlaceholderText('至少12个字符');
    fireEvent.change(passwordInput, { target: { value: 'StrongPass123!' } });
    expect(screen.getByText('强')).toBeInTheDocument();
  });

  it('中等密码应显示"中等"', () => {
    render(<PasswordSetup />);
    const passwordInput = screen.getByPlaceholderText('至少12个字符');
    fireEvent.change(passwordInput, { target: { value: 'Pass1234' } });
    expect(screen.getByText('中等')).toBeInTheDocument();
  });

  it('密码输入框应支持显示/隐藏切换', () => {
    render(<PasswordSetup />);
    const toggleBtns = screen.getAllByLabelText('显示密码');
    expect(toggleBtns).toHaveLength(2);
    fireEvent.click(toggleBtns[0]);
    expect(screen.getByLabelText('隐藏密码')).toBeInTheDocument();
  });

  // ===== 错误路径测试 =====

  it('test_failure_short_password_shows_error', async () => {
    render(<PasswordSetup />);
    const passwordInput = screen.getByPlaceholderText('至少12个字符');
    const confirmInput = screen.getByPlaceholderText('再次输入密码');

    fireEvent.change(passwordInput, { target: { value: 'short' } });
    fireEvent.change(confirmInput, { target: { value: 'short' } });
    fireEvent.click(screen.getByText('创建密码'));

    await waitFor(() => {
      expect(screen.getByText('密码必须至少12个字符')).toBeInTheDocument();
    });
  });

  it('test_failure_password_mismatch_shows_error', async () => {
    render(<PasswordSetup />);
    const passwordInput = screen.getByPlaceholderText('至少12个字符');
    const confirmInput = screen.getByPlaceholderText('再次输入密码');

    fireEvent.change(passwordInput, { target: { value: 'StrongPass123!' } });
    fireEvent.change(confirmInput, { target: { value: 'DifferentPass1!' } });
    fireEvent.click(screen.getByText('创建密码'));

    await waitFor(() => {
      expect(screen.getByText('两次输入的密码不一致')).toBeInTheDocument();
    });
  });

  it('test_failure_missing_uppercase_shows_error', async () => {
    render(<PasswordSetup />);
    const passwordInput = screen.getByPlaceholderText('至少12个字符');
    const confirmInput = screen.getByPlaceholderText('再次输入密码');

    fireEvent.change(passwordInput, { target: { value: 'alllowercase1' } });
    fireEvent.change(confirmInput, { target: { value: 'alllowercase1' } });
    fireEvent.click(screen.getByText('创建密码'));

    await waitFor(() => {
      expect(screen.getByText('密码必须包含大小写字母和数字')).toBeInTheDocument();
    });
  });

  it('test_failure_api_error_shows_message', async () => {
    const { authApi } = await import('../../../utils/tauri');
    vi.mocked(authApi.setupMasterPassword).mockRejectedValueOnce(
      new Error('Server error'),
    );

    render(<PasswordSetup />);
    const passwordInput = screen.getByPlaceholderText('至少12个字符');
    const confirmInput = screen.getByPlaceholderText('再次输入密码');

    fireEvent.change(passwordInput, { target: { value: 'StrongPass123!' } });
    fireEvent.change(confirmInput, { target: { value: 'StrongPass123!' } });
    fireEvent.click(screen.getByText('创建密码'));

    await waitFor(() => {
      expect(screen.getByText(/错误/)).toBeInTheDocument();
    });
  });

  // ===== 契约测试 =====

  it('test_contract_renders_without_props', () => {
    expect(() => {
      render(<PasswordSetup />);
    }).not.toThrow();
  });

  it('test_contract_create_button_exists', () => {
    render(<PasswordSetup />);
    expect(screen.getByText('创建密码')).toBeInTheDocument();
  });

  // ===== 安全审计测试 =====

  it('test_audit_password_inputs_are_masked_by_default', () => {
    render(<PasswordSetup />);
    const passwordInput = screen.getByPlaceholderText('至少12个字符');
    const confirmInput = screen.getByPlaceholderText('再次输入密码');
    expect(passwordInput).toHaveAttribute('type', 'password');
    expect(confirmInput).toHaveAttribute('type', 'password');
  });

  it('test_audit_no_password_value_in_dom', () => {
    const { container } = render(<PasswordSetup />);
    const passwordInput = screen.getByPlaceholderText('至少12个字符');
    fireEvent.change(passwordInput, { target: { value: 'SuperSecret123!' } });

    // 密码不应出现在 DOM 的非 input 元素中
    const allText = Array.from(container.querySelectorAll('*'))
      .filter((el) => el.tagName !== 'INPUT')
      .map((el) => el.textContent)
      .join('');
    expect(allText).not.toContain('SuperSecret123!');
  });

  it('test_audit_strength_indicator_does_not_leak_password', () => {
    render(<PasswordSetup />);
    const passwordInput = screen.getByPlaceholderText('至少12个字符');
    fireEvent.change(passwordInput, { target: { value: 'MyPassword123' } });

    // 强度指示器只显示"强"/"中等"/"弱"，不显示密码
    const strengthText = screen.getByText('强');
    expect(strengthText.textContent).not.toContain('MyPassword');
  });
});
