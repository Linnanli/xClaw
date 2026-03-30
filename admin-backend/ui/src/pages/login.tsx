import { useState, type FormEvent } from 'react'
import { useNavigate } from 'react-router-dom'
import { Input } from '@/components/ui/input'
import { Checkbox } from '@/components/ui/checkbox'
import {
  User, Lock, EyeOff, Smartphone, LogIn,
  Server, KeyRound, ShieldCheck,
} from 'lucide-react'
import { useAuthStore } from '@/stores/auth'

export default function LoginPage() {
  const navigate = useNavigate()
  const login = useAuthStore((s) => s.login)

  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [mfaCode, setMfaCode] = useState('')
  const [rememberMe, setRememberMe] = useState(false)
  const [error, setError] = useState('')
  const [loading, setLoading] = useState(false)

  async function handleSubmit(e: FormEvent) {
    e.preventDefault()
    setError('')
    if (!username.trim()) { setError('请输入用户名'); return }
    if (!password.trim()) { setError('请输入密码'); return }

    setLoading(true)
    try {
      await login({
        username: username.trim(),
        password,
        mfa_code: mfaCode || undefined,
        remember_me: rememberMe,
      })
      navigate('/', { replace: true })
    } catch {
      setError('用户名或密码错误')
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="flex h-screen w-screen">
      {/* ===== 左侧品牌面板 ===== */}
      <div
        className="hidden lg:flex w-[640px] shrink-0 flex-col justify-between p-[60px]"
        style={{ backgroundColor: '#F0F7F2', borderRight: '1px solid #E0E0E0' }}
      >
        {/* 上部内容 */}
        <div className="flex flex-col gap-12">
          {/* Logo */}
          <div className="flex items-center gap-3">
            <div className="flex h-10 w-10 items-center justify-center bg-[#0A6B3A]">
              <span className="text-white font-mono text-base font-bold">IC</span>
            </div>
            <span className="font-mono text-lg font-semibold tracking-[2px] text-[#1A1A1A]">
              IRONCLAW
            </span>
          </div>

          {/* 大标题 */}
          <div className="flex flex-col gap-5">
            <h1
              className="text-[56px] font-bold leading-none tracking-tight text-[#1A1A1A]"
              style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px' }}
            >
              企业级{'\n'}AI 助手{'\n'}管理平台
            </h1>
            <p className="font-mono text-xs leading-[1.8] text-[#666666] max-w-[420px]">
              统一管控 DLP 策略、用户权限与客户端{'\n'}让数据安全与智能协作同步推进
            </p>
          </div>

          {/* 统计数字 */}
          <div className="flex gap-8">
            <div className="flex flex-col gap-1.5">
              <span className="text-[32px] font-bold tracking-tight text-[#0A6B3A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>
                24
              </span>
              <span className="font-mono text-[9px] font-medium tracking-[1px] text-[#888888]">
                DLP 规则
              </span>
            </div>
            <div className="flex flex-col gap-1.5">
              <span className="text-[32px] font-bold tracking-tight text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>
                89
              </span>
              <span className="font-mono text-[9px] font-medium tracking-[1px] text-[#888888]">
                接入客户端
              </span>
            </div>
            <div className="flex flex-col gap-1.5">
              <span className="text-[32px] font-bold tracking-tight text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>
                99.97%
              </span>
              <span className="font-mono text-[9px] font-medium tracking-[1px] text-[#888888]">
                系统可用率
              </span>
            </div>
          </div>
        </div>

        {/* 底部状态 */}
        <div className="flex items-center gap-2">
          <div className="h-1.5 w-1.5 rounded-full bg-[#0A6B3A]" />
          <span className="font-mono text-[10px] font-medium tracking-[0.5px] text-[#888888]">
            所有服务运行正常
          </span>
        </div>
      </div>

      {/* ===== 右侧登录表单 ===== */}
      <div className="flex flex-1 items-center justify-center bg-white px-20">
        <form onSubmit={handleSubmit} className="flex w-[400px] flex-col gap-7">
          {/* 表单头部 */}
          <div className="flex flex-col gap-3">
            <h2
              className="text-4xl font-bold tracking-tight text-[#1A1A1A]"
              style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px' }}
            >
              管理员登录
            </h2>
            <p className="font-mono text-xs text-[#666666]">
              请输入凭据以访问管理控制台
            </p>
          </div>

          {/* 错误提示 */}
          {error && (
            <div className="rounded bg-red-50 px-3 py-2 font-mono text-xs text-[#CF1322]">
              {error}
            </div>
          )}

          {/* 表单字段 */}
          <div className="flex flex-col gap-4">
            {/* 用户名 */}
            <div className="flex flex-col gap-2">
              <label className="font-mono text-[9px] font-semibold tracking-[1px] text-[#888888]">
                用户名
              </label>
              <div className="flex items-center gap-3 border border-[#D9D9D9] bg-white px-4 py-3.5">
                <User className="h-3.5 w-3.5 text-[#999999]" />
                <Input
                  className="border-0 p-0 h-auto font-mono text-[11px] shadow-none focus-visible:ring-0 placeholder:text-[#BFBFBF]"
                  placeholder="请输入用户名"
                  value={username}
                  onChange={(e) => setUsername(e.target.value)}
                  autoComplete="username"
                />
              </div>
            </div>

            {/* 密码 */}
            <div className="flex flex-col gap-2">
              <label className="font-mono text-[9px] font-semibold tracking-[1px] text-[#888888]">
                密码
              </label>
              <div className="flex items-center justify-between border border-[#D9D9D9] bg-white px-4 py-3.5">
                <div className="flex items-center gap-3 flex-1">
                  <Lock className="h-3.5 w-3.5 text-[#999999]" />
                  <Input
                    type="password"
                    className="border-0 p-0 h-auto font-mono text-[11px] shadow-none focus-visible:ring-0 placeholder:text-[#BFBFBF]"
                    placeholder="请输入密码"
                    value={password}
                    onChange={(e) => setPassword(e.target.value)}
                    autoComplete="current-password"
                  />
                </div>
                <EyeOff className="h-3.5 w-3.5 text-[#999999] shrink-0" />
              </div>
            </div>

            {/* MFA 验证码 */}
            <div className="flex flex-col gap-2">
              <div className="flex items-center gap-2">
                <label className="font-mono text-[9px] font-semibold tracking-[1px] text-[#888888]">
                  MFA 验证码
                </label>
                <span className="rounded border border-[#0A6B3A] bg-[#E8F5E9] px-1.5 py-0.5 font-mono text-[7px] font-semibold text-[#0A6B3A]">
                  新增
                </span>
              </div>
              <div className="flex items-center gap-3 border border-[#D9D9D9] bg-white px-4 py-3.5">
                <Smartphone className="h-3.5 w-3.5 text-[#999999]" />
                <Input
                  className="border-0 p-0 h-auto font-mono text-[11px] shadow-none focus-visible:ring-0 placeholder:text-[#BFBFBF]"
                  placeholder="6 位 TOTP 验证码"
                  value={mfaCode}
                  onChange={(e) => setMfaCode(e.target.value)}
                  maxLength={6}
                />
              </div>
            </div>

            {/* 记住我 */}
            <div className="flex items-center gap-2.5">
              <Checkbox
                id="remember"
                checked={rememberMe}
                onCheckedChange={(checked) => setRememberMe(checked === true)}
                className="h-4 w-4 border-[#D9D9D9]"
              />
              <label htmlFor="remember" className="font-mono text-[10px] font-medium tracking-[0.5px] text-[#666666] cursor-pointer">
                记住我
              </label>
            </div>
          </div>

          {/* 登录按钮 */}
          <button
            type="submit"
            disabled={loading}
            className="flex w-full items-center justify-center gap-2.5 bg-[#0A6B3A] py-4 text-white disabled:opacity-60"
          >
            <LogIn className="h-3.5 w-3.5" />
            <span className="font-mono text-xs font-bold tracking-[1px]">
              {loading ? '登录中...' : '登录'}
            </span>
          </button>

          {/* SSO 分隔线 */}
          <div className="flex items-center gap-3">
            <div className="h-px flex-1 bg-[#E0E0E0]" />
            <span className="font-mono text-[9px] font-medium text-[#999999]">
              或通过 SSO 登录
            </span>
            <div className="h-px flex-1 bg-[#E0E0E0]" />
          </div>

          {/* SSO 按钮 */}
          <div className="flex gap-3">
            <button
              type="button"
              className="flex flex-1 items-center justify-center gap-2 border border-[#D9D9D9] bg-white py-3"
            >
              <Server className="h-3 w-3 text-[#999999]" />
              <span className="font-mono text-[9px] font-semibold text-[#1A1A1A]">
                LDAP / AD
              </span>
            </button>
            <button
              type="button"
              className="flex flex-1 items-center justify-center gap-2 border border-[#D9D9D9] bg-white py-3"
            >
              <KeyRound className="h-3 w-3 text-[#999999]" />
              <span className="font-mono text-[9px] font-semibold text-[#1A1A1A]">
                OIDC / SAML
              </span>
            </button>
          </div>

          {/* 安全提示 */}
          <div
            className="flex items-center gap-2.5 px-4 py-3"
            style={{ backgroundColor: '#F6FFF8', border: '1px solid rgba(10, 107, 58, 0.19)' }}
          >
            <ShieldCheck className="h-3 w-3 shrink-0 text-[#0A6B3A]" />
            <span className="font-mono text-[8px] font-medium text-[#888888]">
              TLS 1.3 加密传输 · 会话 8 小时后过期 · 5 次失败锁定 15 分钟
            </span>
          </div>
        </form>
      </div>
    </div>
  )
}
