import { useNavigate } from 'react-router-dom'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'
import {
  User, Lock, EyeOff, Smartphone, LogIn,
  Server, KeyRound, ShieldCheck, type LucideIcon,
} from 'lucide-react'
import { useAuthStore } from '@/stores/auth'
import {
  Form, FormControl, FormField, FormItem, FormMessage,
} from '@/components/ui/form'
import { Input } from '@/components/ui/input'
import { Checkbox } from '@/components/ui/checkbox'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'

const loginSchema = z.object({
  username: z.string().min(1, '请输入用户名').trim(),
  password: z.string().min(1, '请输入密码'),
  mfaCode: z.string().optional(),
  rememberMe: z.boolean().default(false),
})

type LoginFormValues = z.infer<typeof loginSchema>

const BRAND_STATS = [
  { value: '24', label: 'DLP 规则', color: '#0A6B3A' },
  { value: '89', label: '接入客户端', color: '#1A1A1A' },
  { value: '99.97%', label: '系统可用率', color: '#1A1A1A' },
] as const

type FieldInputProps = {
  icon: LucideIcon
  children: React.ReactNode
  suffix?: React.ReactNode
}

function FieldInput({ icon: Icon, children, suffix }: FieldInputProps) {
  return (
    <div className="flex items-center justify-between border border-[#D9D9D9] bg-white px-4 py-3.5">
      <div className="flex items-center gap-3 flex-1">
        <Icon className="h-3.5 w-3.5 shrink-0 text-[#999999]" />
        {children}
      </div>
      {suffix}
    </div>
  )
}

const inputClass = "border-0 p-0 h-auto font-mono text-[11px] shadow-none focus-visible:ring-0 placeholder:text-[#BFBFBF]"

export default function LoginPage() {
  const navigate = useNavigate()
  const login = useAuthStore((s) => s.login)

  const form = useForm<LoginFormValues>({
    resolver: zodResolver(loginSchema),
    defaultValues: { username: '', password: '', mfaCode: '', rememberMe: false },
  })

  async function onSubmit(values: LoginFormValues) {
    try {
      await login({
        username: values.username,
        password: values.password,
        mfa_code: values.mfaCode || undefined,
        remember_me: values.rememberMe,
      })
      navigate('/', { replace: true })
    } catch {
      form.setError('root', { message: '用户名或密码错误' })
    }
  }

  return (
    <div className="flex h-screen w-screen">
      {/* 左侧品牌面板 */}
      <div
        className="hidden lg:flex w-[640px] shrink-0 flex-col justify-between p-[60px]"
        style={{ backgroundColor: '#F0F7F2', borderRight: '1px solid #E0E0E0' }}
      >
        <div className="flex flex-col gap-12">
          <div className="flex items-center gap-3">
            <div className="flex h-10 w-10 items-center justify-center bg-[#0A6B3A]">
              <span className="text-white font-mono text-base font-bold">IC</span>
            </div>
            <span className="font-mono text-lg font-semibold tracking-[2px] text-[#1A1A1A]">
              IRONCLAW
            </span>
          </div>

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

          <div className="flex gap-8">
            {BRAND_STATS.map((stat) => (
              <div key={stat.label} className="flex flex-col gap-1.5">
                <span
                  className="text-[32px] font-bold tracking-tight"
                  style={{ fontFamily: 'Space Grotesk, sans-serif', color: stat.color }}
                >
                  {stat.value}
                </span>
                <span className="font-mono text-[9px] font-medium tracking-[1px] text-[#888888]">
                  {stat.label}
                </span>
              </div>
            ))}
          </div>
        </div>

        <div className="flex items-center gap-2">
          <div className="h-1.5 w-1.5 rounded-full bg-[#0A6B3A]" />
          <span className="font-mono text-[10px] font-medium tracking-[0.5px] text-[#888888]">
            所有服务运行正常
          </span>
        </div>
      </div>

      {/* 右侧登录表单 */}
      <div className="flex flex-1 items-center justify-center bg-white px-20">
        <div className="flex w-[400px] flex-col gap-7">
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

          <Form {...form}>
            <form onSubmit={form.handleSubmit(onSubmit)} className="flex flex-col gap-4">
              {form.formState.errors.root && (
                <div className="rounded bg-red-50 px-3 py-2 font-mono text-xs text-[#CF1322]">
                  {form.formState.errors.root.message}
                </div>
              )}

              <FormField
                control={form.control}
                name="username"
                render={({ field }) => (
                  <FormItem>
                    <Label className="font-mono text-[9px] font-semibold tracking-[1px] text-[#888888]">用户名</Label>
                    <FormControl>
                      <FieldInput icon={User}>
                        <Input {...field} className={inputClass} placeholder="请输入用户名" autoComplete="username" />
                      </FieldInput>
                    </FormControl>
                    <FormMessage className="font-mono text-[10px]" />
                  </FormItem>
                )}
              />

              <FormField
                control={form.control}
                name="password"
                render={({ field }) => (
                  <FormItem>
                    <Label className="font-mono text-[9px] font-semibold tracking-[1px] text-[#888888]">密码</Label>
                    <FormControl>
                      <FieldInput icon={Lock} suffix={<EyeOff className="h-3.5 w-3.5 text-[#999999] shrink-0" />}>
                        <Input {...field} type="password" className={inputClass} placeholder="请输入密码" autoComplete="current-password" />
                      </FieldInput>
                    </FormControl>
                    <FormMessage className="font-mono text-[10px]" />
                  </FormItem>
                )}
              />

              <FormField
                control={form.control}
                name="mfaCode"
                render={({ field }) => (
                  <FormItem>
                    <div className="flex items-center gap-2">
                      <Label className="font-mono text-[9px] font-semibold tracking-[1px] text-[#888888]">MFA 验证码</Label>
                      <span className="rounded border border-[#0A6B3A] bg-[#E8F5E9] px-1.5 py-0.5 font-mono text-[7px] font-semibold text-[#0A6B3A]">
                        新增
                      </span>
                    </div>
                    <FormControl>
                      <FieldInput icon={Smartphone}>
                        <Input {...field} className={inputClass} placeholder="6 位 TOTP 验证码" maxLength={6} />
                      </FieldInput>
                    </FormControl>
                  </FormItem>
                )}
              />

              <FormField
                control={form.control}
                name="rememberMe"
                render={({ field }) => (
                  <FormItem className="flex-row items-center gap-2.5">
                    <FormControl>
                      <Checkbox
                        id="rememberMe"
                        checked={field.value}
                        onCheckedChange={field.onChange}
                        className="h-4 w-4 border-[#D9D9D9]"
                      />
                    </FormControl>
                    <Label htmlFor="rememberMe" className="font-mono text-[10px] font-medium tracking-[0.5px] text-[#666666] cursor-pointer">
                      记住我
                    </Label>
                  </FormItem>
                )}
              />

              <Button
                type="submit"
                disabled={form.formState.isSubmitting}
                className="flex w-full items-center justify-center gap-2.5 bg-[#0A6B3A] py-4 text-white hover:bg-[#085c31] disabled:opacity-60 rounded-none h-auto"
              >
                <LogIn className="h-3.5 w-3.5" />
                <span className="font-mono text-xs font-bold tracking-[1px]">
                  {form.formState.isSubmitting ? '登录中...' : '登录'}
                </span>
              </Button>
            </form>
          </Form>

          <div className="flex items-center gap-3">
            <div className="h-px flex-1 bg-[#E0E0E0]" />
            <span className="font-mono text-[9px] font-medium text-[#999999]">或通过 SSO 登录</span>
            <div className="h-px flex-1 bg-[#E0E0E0]" />
          </div>

          <div className="flex gap-3">
            {[
              { icon: Server, label: 'LDAP / AD' },
              { icon: KeyRound, label: 'OIDC / SAML' },
            ].map(({ icon: Icon, label }) => (
              <Button
                key={label}
                type="button"
                variant="outline"
                className="flex flex-1 items-center justify-center gap-2 border border-[#D9D9D9] bg-white py-3 rounded-none h-auto hover:bg-[#F5F5F5]"
              >
                <Icon className="h-3 w-3 text-[#999999]" />
                <span className="font-mono text-[9px] font-semibold text-[#1A1A1A]">{label}</span>
              </Button>
            ))}
          </div>

          <div
            className="flex items-center gap-2.5 px-4 py-3"
            style={{ backgroundColor: '#F6FFF8', border: '1px solid rgba(10, 107, 58, 0.19)' }}
          >
            <ShieldCheck className="h-3 w-3 shrink-0 text-[#0A6B3A]" />
            <span className="font-mono text-[8px] font-medium text-[#888888]">
              TLS 1.3 加密传输 · 会话 8 小时后过期 · 5 次失败锁定 15 分钟
            </span>
          </div>
        </div>
      </div>
    </div>
  )
}
