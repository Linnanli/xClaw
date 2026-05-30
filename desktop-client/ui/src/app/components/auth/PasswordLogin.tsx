/**
 * PasswordLogin - 登录页面
 *
 * 设计稿：左侧绿色品牌面板（560px）+ 右侧白色登录表单。
 * 左侧：Shield 图标 + X-Claw 标题 + 标语。
 * 右侧：欢迎登录 + 邮箱/密码表单 + 登录按钮 + SSO 登录。
 */

import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router';
import { Eye, EyeOff, LogIn, Building2, Shield } from 'lucide-react';
import { Button } from '../ui/button';
import { Input } from '../ui/input';
import { Label } from '../ui/label';
import { authApi } from '../../utils/tauri';

export function PasswordLogin() {
  const navigate = useNavigate();
  const [password, setPassword] = useState('');
  const [showPassword, setShowPassword] = useState(false);
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    const sessionId = sessionStorage.getItem('session_id');
    if (sessionId) {
      navigate('/app');
      return;
    }

    authApi
      .checkSetupStatus()
      .then(async (status) => {
        if (!status.password_set) {
          navigate('/setup');
          return;
        }
        try {
          const result = await authApi.unlockApp('');
          if (result.success && result.session_id) {
            sessionStorage.setItem('session_id', result.session_id);
            navigate('/app');
          }
        } catch {
          // 非嵌入式模式：保持密码输入页面
        }
      })
      .catch((err) => {
        console.error('Failed to check setup status:', err);
      });
  }, [navigate]);

  const handleUnlock = async () => {
    setError('');
    setLoading(true);

    if (!password) {
      setError('请输入密码');
      setLoading(false);
      return;
    }

    try {
      const result = await authApi.unlockApp(password);
      if (result.success && result.session_id) {
        sessionStorage.setItem('session_id', result.session_id);
        navigate('/app');
      } else {
        setError(result.message || '认证失败');
      }
    } catch (err: any) {
      setError('错误：' + err.message);
    } finally {
      setLoading(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') handleUnlock();
  };

  return (
    <div className="flex h-screen w-screen">
      {/* 左侧品牌面板 */}
      <div className="hidden w-[560px] shrink-0 flex-col items-center justify-center gap-6 bg-primary p-[60px] lg:flex">
        <div className="flex items-center gap-3">
          <Shield className="size-9 text-primary-foreground" />
          <span className="text-[32px] font-extrabold text-primary-foreground">X-Claw</span>
        </div>
        <p className="text-lg font-medium text-primary-foreground/90">政企级 AI 智能助手</p>
        <div className="h-0.5 w-10 rounded-full bg-primary-foreground/30" />
        <p className="text-sm text-primary-foreground/70">安全 · 合规 · 可控</p>
      </div>

      {/* 右侧登录表单 */}
      <div className="flex flex-1 items-center justify-center bg-background px-[120px] py-[60px]">
        <div className="flex w-full max-w-[400px] flex-col gap-6">
          <div>
            <h1 className="text-[28px] font-bold text-foreground">欢迎登录</h1>
            <p className="mt-1 text-sm text-muted-foreground">请使用企业账号登录 X-Claw</p>
          </div>

          {/* 邮箱字段 */}
          <div className="space-y-1.5">
            <Label className="text-[13px] font-semibold text-foreground/80">企业邮箱</Label>
            <Input
              type="email"
              placeholder="name@company.com"
              className="h-11 rounded-[10px] border-border bg-background px-3.5"
            />
          </div>

          {/* 密码字段 */}
          <div className="space-y-1.5">
            <Label className="text-[13px] font-semibold text-foreground/80">密码</Label>
            <div className="relative">
              <Input
                type={showPassword ? 'text' : 'password'}
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                onKeyDown={handleKeyDown}
                placeholder="输入您的密码"
                className="h-11 rounded-[10px] border-border bg-background px-3.5 pr-10"
                autoFocus
              />
              <button
                type="button"
                onClick={() => setShowPassword(!showPassword)}
                className="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground transition-colors hover:text-foreground"
                aria-label={showPassword ? '隐藏密码' : '显示密码'}
              >
                {showPassword ? <EyeOff className="size-5" /> : <Eye className="size-5" />}
              </button>
            </div>
          </div>

          {/* 错误提示 */}
          {error && (
            <div className="rounded-lg border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
              {error}
            </div>
          )}

          {/* 登录按钮 */}
          <Button
            onClick={handleUnlock}
            disabled={loading}
            className="h-11 w-full rounded-[10px] gap-2 text-[15px] font-semibold"
          >
            <LogIn className="size-4" />
            {loading ? '登录中...' : '登录'}
          </Button>

          {/* 分隔线 */}
          <div className="flex items-center gap-3">
            <div className="h-px flex-1 bg-border" />
            <span className="text-xs text-muted-foreground">或</span>
            <div className="h-px flex-1 bg-border" />
          </div>

          {/* SSO 登录 */}
          <Button
            variant="outline"
            className="h-11 w-full rounded-[10px] gap-2 text-sm font-medium"
            onClick={() => navigate('/setup')}
          >
            <Building2 className="size-4" />
            企业 SSO 登录
          </Button>
        </div>
      </div>
    </div>
  );
}
