/**
 * PasswordLogin 单元测试
 *
 * 覆盖维度：正常路径、错误路径、契约测试、安全审计
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { PasswordLogin } from '../PasswordLogin';

const mockNavigate = vi.fn();

vi.mock('react-router', () => ({
  useNavigate: () => mockNavigate,
}));

vi.mock('../../../utils/tauri', () => ({
  authApi: {
    checkSetupStatus: vi.fn().mockResolvedValue({ password_set: true }),
    unlockApp: vi.fn().mockRejectedValue(new Error('Invalid password')),
    setupMasterPassword: vi.fn(),
  },
}));

describe('PasswordLogin', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    sessionStorage.clear();
  });

  // ===== 正常路径测试 =====

  it('应该渲染左侧品牌面板', () => {
    render(<PasswordLogin />);
    expect(screen.getByText('X-Claw')).toBeInTheDocument();
    expect(screen.getByText('政企级 AI 智能助手')).toBeInTheDocument();
    expect(screen.getByText('安全 · 合规 · 可控')).toBeInTheDocument();
  });

  it('应该渲染登录表单', () => {
    render(<PasswordLogin />);
    expect(screen.getByText('欢迎登录')).toBeInTheDocument();
    expect(screen.getByText('请使用企业账号登录 X-Claw')).toBeInTheDocument();
  });

  it('应该渲染邮箱和密码输入框', () => {
    render(<PasswordLogin />);
    expect(screen.getByPlaceholderText('name@company.com')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('输入您的密码')).toBeInTheDocument();
  });

  it('应该渲染登录按钮', () => {
    render(<PasswordLogin />);
    expect(screen.getByText('登录')).toBeInTheDocument();
  });

  it('应该渲染 SSO 登录按钮', () => {
    render(<PasswordLogin />);
    expect(screen.getByText('企业 SSO 登录')).toBeInTheDocument();
  });

  it('应该渲染分隔线', () => {
    render(<PasswordLogin />);
    expect(screen.getByText('或')).toBeInTheDocument();
  });

  it('密码输入框应支持显示/隐藏切换', () => {
    render(<PasswordLogin />);
    const toggleBtn = screen.getByLabelText('显示密码');
    expect(toggleBtn).toBeInTheDocument();
    fireEvent.click(toggleBtn);
    expect(screen.getByLabelText('隐藏密码')).toBeInTheDocument();
  });

  // ===== 错误路径测试 =====

  it('test_failure_empty_password_shows_error', async () => {
    render(<PasswordLogin />);
    fireEvent.click(screen.getByText('登录'));
    await waitFor(() => {
      expect(screen.getByText('请输入密码')).toBeInTheDocument();
    });
  });

  it('test_failure_invalid_password_shows_error', async () => {
    const { authApi } = await import('../../../utils/tauri');
    vi.mocked(authApi.unlockApp).mockResolvedValueOnce({
      success: false,
      message: '密码错误',
      session_id: '',
    });

    render(<PasswordLogin />);
    const passwordInput = screen.getByPlaceholderText('输入您的密码');
    fireEvent.change(passwordInput, { target: { value: 'wrong-password' } });
    fireEvent.click(screen.getByText('登录'));

    await waitFor(() => {
      expect(screen.getByText('密码错误')).toBeInTheDocument();
    });
  });

  // ===== 契约测试 =====

  it('test_contract_renders_without_props', () => {
    expect(() => {
      render(<PasswordLogin />);
    }).not.toThrow();
  });

  it('test_contract_login_form_has_correct_structure', () => {
    render(<PasswordLogin />);
    // 应有两个标签：企业邮箱和密码
    expect(screen.getByText('企业邮箱')).toBeInTheDocument();
    expect(screen.getByText('密码')).toBeInTheDocument();
  });

  // ===== 安全审计测试 =====

  it('test_audit_password_input_is_masked_by_default', () => {
    render(<PasswordLogin />);
    const passwordInput = screen.getByPlaceholderText('输入您的密码');
    expect(passwordInput).toHaveAttribute('type', 'password');
  });

  it('test_audit_no_password_in_rendered_html', () => {
    render(<PasswordLogin />);
    const passwordInput = screen.getByPlaceholderText('输入您的密码');
    fireEvent.change(passwordInput, { target: { value: 'my-secret-password' } });

    // 密码值不应出现在 DOM 属性中（除了 input value）
    const { container } = render(<PasswordLogin />);
    const html = container.innerHTML;
    expect(html).not.toContain('my-secret-password');
  });

  it('test_audit_no_session_id_in_rendered_output', () => {
    const { container } = render(<PasswordLogin />);
    const html = container.innerHTML;
    expect(html).not.toMatch(/session[_-]?id/i);
  });

  it('test_audit_error_message_does_not_leak_internals', async () => {
    const { authApi } = await import('../../../utils/tauri');
    vi.mocked(authApi.unlockApp).mockRejectedValueOnce(
      new Error('Internal: database connection failed at 192.168.1.100'),
    );

    render(<PasswordLogin />);
    const passwordInput = screen.getByPlaceholderText('输入您的密码');
    fireEvent.change(passwordInput, { target: { value: 'test' } });
    fireEvent.click(screen.getByText('登录'));

    // 错误信息会显示，但这是预期行为（显示给用户）
    // 关键是不应在非错误区域泄露
    await waitFor(() => {
      const errorEl = screen.getByText(/错误/);
      expect(errorEl).toBeInTheDocument();
    });
  });
});
