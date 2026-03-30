import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router-dom'
import LoginPage from '@/pages/login'
import { useAuthStore } from '@/stores/auth'

const mockNavigate = vi.fn()
vi.mock('react-router-dom', async () => {
  const actual = await vi.importActual('react-router-dom')
  return { ...actual, useNavigate: () => mockNavigate }
})

function renderLogin() {
  return render(
    <MemoryRouter>
      <LoginPage />
    </MemoryRouter>
  )
}

describe('LoginPage', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    useAuthStore.setState({ token: null, user: null, isAuthenticated: false })
  })

  it('应渲染登录表单核心元素', () => {
    renderLogin()
    expect(screen.getByText('管理员登录')).toBeInTheDocument()
    expect(screen.getByPlaceholderText('请输入用户名')).toBeInTheDocument()
    expect(screen.getByPlaceholderText('请输入密码')).toBeInTheDocument()
    expect(screen.getByPlaceholderText('6 位 TOTP 验证码')).toBeInTheDocument()
    expect(screen.getByText('登录')).toBeInTheDocument()
  })

  it('应渲染左侧品牌区内容', () => {
    renderLogin()
    expect(screen.getByText('IRONCLAW')).toBeInTheDocument()
    expect(screen.getByText('所有服务运行正常')).toBeInTheDocument()
  })

  it('应渲染 SSO 按钮和安全提示', () => {
    renderLogin()
    expect(screen.getByText('LDAP / AD')).toBeInTheDocument()
    expect(screen.getByText('OIDC / SAML')).toBeInTheDocument()
    expect(screen.getByText(/TLS 1.3/)).toBeInTheDocument()
  })

  it('空用户名提交时应显示验证错误', async () => {
    renderLogin()
    const user = userEvent.setup()
    await user.click(screen.getByText('登录'))
    expect(screen.getByText('请输入用户名')).toBeInTheDocument()
  })

  it('空密码提交时应显示验证错误', async () => {
    renderLogin()
    const user = userEvent.setup()
    await user.type(screen.getByPlaceholderText('请输入用户名'), 'admin')
    await user.click(screen.getByText('登录'))
    expect(screen.getByText('请输入密码')).toBeInTheDocument()
  })

  it('登录成功后应跳转到首页', async () => {
    const loginMock = vi.fn().mockResolvedValue(undefined)
    useAuthStore.setState({ login: loginMock })

    renderLogin()
    const user = userEvent.setup()
    await user.type(screen.getByPlaceholderText('请输入用户名'), 'admin')
    await user.type(screen.getByPlaceholderText('请输入密码'), 'password123')
    await user.click(screen.getByText('登录'))

    await waitFor(() => {
      expect(loginMock).toHaveBeenCalledWith({
        username: 'admin',
        password: 'password123',
        mfa_code: undefined,
        remember_me: false,
      })
      expect(mockNavigate).toHaveBeenCalledWith('/', { replace: true })
    })
  })

  it('登录失败时应显示错误信息', async () => {
    const loginMock = vi.fn().mockRejectedValue(new Error('fail'))
    useAuthStore.setState({ login: loginMock })

    renderLogin()
    const user = userEvent.setup()
    await user.type(screen.getByPlaceholderText('请输入用户名'), 'admin')
    await user.type(screen.getByPlaceholderText('请输入密码'), 'wrong')
    await user.click(screen.getByText('登录'))

    await waitFor(() => {
      expect(screen.getByText('用户名或密码错误')).toBeInTheDocument()
    })
  })
})
