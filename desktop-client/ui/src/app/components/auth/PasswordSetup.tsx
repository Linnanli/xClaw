/**
 * PasswordSetup - 密码设置页面
 *
 * 设计稿：与登录页相同的左右分栏布局。
 * 左侧绿色品牌面板 + 右侧密码设置表单。
 */

import { useState } from 'react';
import { useNavigate } from 'react-router';
import { Eye, EyeOff, Shield, Check } from 'lucide-react';
import { Button } from '../ui/button';
import { Input } from '../ui/input';
import { Label } from '../ui/label';
import { cn } from '../ui/utils';
import { authApi } from '../../utils/tauri';

function getPasswordStrength(pwd: string): '弱' | '中等' | '强' {
  if (pwd.length < 8) return '弱';
  const hasUpper = /[A-Z]/.test(pwd);
  const hasLower = /[a-z]/.test(pwd);
  const hasNumber = /[0-9]/.test(pwd);
  if (hasUpper && hasLower && hasNumber && pwd.length >= 12) return '强';
  if ((hasUpper || hasLower) && hasNumber && pwd.length >= 8) return '中等';
  return '弱';
}

const STRENGTH_CONFIG = {
  '弱': { width: '33%', color: 'bg-destructive' },
  '中等': { width: '66%', color: 'bg-yellow-500' },
  '强': { width: '100%', color: 'bg-primary' },
};

export function PasswordSetup() {
  const navigate = useNavigate();
  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [showPassword, setShowPassword] = useState(false);
  const [showConfirmPassword, setShowConfirmPassword] = useState(false);
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);

  const strength = getPasswordStrength(password);
  const strengthCfg = STRENGTH_CONFIG[strength];

  const requirements = [
    { label: '至少12个字符', met: password.length >= 12 },
    { label: '包含大写字母', met: /[A-Z]/.test(password) },
    { label: '包含小写字母', met: /[a-z]/.test(password) },
    { label: '包含数字', met: /[0-9]/.test(password) },
  ];

  const handleCreate = async () => {
    setError('');
    setLoading(true);

    if (password.length < 12) {
      setError('密码必须至少12个字符');
      setLoading(false);
      return;
    }

    if (!requirements.every((r) => r.met)) {
      setError('密码必须包含大小写字母和数字');
      setLoading(false);
      return;
    }

    if (password !== confirmPassword) {
      setError('两次输入的密码不一致');
      setLoading(false);
      return;
    }

    try {
      const result = await authApi.setupMasterPassword(password);
      if (result.success) {
        navigate('/');
      } else {
        setError(result.message || '设置密码失败');
      }
    } catch (err: any) {
      setError('错误：' + err.message);
    } finally {
      setLoading(false);
    }
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

      {/* 右侧表单 */}
      <div className="flex flex-1 items-center justify-center bg-background px-[120px] py-[60px]">
        <div className="flex w-full max-w-[400px] flex-col gap-6">
          <div>
            <h1 className="text-[28px] font-bold text-foreground">创建主密码</h1>
            <p className="mt-1 text-sm text-muted-foreground">设置一个安全的主密码来保护您的数据</p>
          </div>

          {/* 密码字段 */}
          <div className="space-y-1.5">
            <Label className="text-[13px] font-semibold text-foreground/80">主密码</Label>
            <div className="relative">
              <Input
                type={showPassword ? 'text' : 'password'}
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder="至少12个字符"
                className="h-11 rounded-[10px] border-border bg-background px-3.5 pr-10"
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

            {/* 密码强度 */}
            {password && (
              <div className="flex items-center gap-2 pt-1">
                <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-secondary">
                  <div
                    className={cn('h-full rounded-full transition-all duration-300', strengthCfg.color)}
                    style={{ width: strengthCfg.width }}
                  />
                </div>
                <span className="text-xs font-medium text-muted-foreground">{strength}</span>
              </div>
            )}
          </div>

          {/* 确认密码 */}
          <div className="space-y-1.5">
            <Label className="text-[13px] font-semibold text-foreground/80">确认密码</Label>
            <div className="relative">
              <Input
                type={showConfirmPassword ? 'text' : 'password'}
                value={confirmPassword}
                onChange={(e) => setConfirmPassword(e.target.value)}
                placeholder="再次输入密码"
                className="h-11 rounded-[10px] border-border bg-background px-3.5 pr-10"
              />
              <button
                type="button"
                onClick={() => setShowConfirmPassword(!showConfirmPassword)}
                className="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground transition-colors hover:text-foreground"
                aria-label={showConfirmPassword ? '隐藏密码' : '显示密码'}
              >
                {showConfirmPassword ? <EyeOff className="size-5" /> : <Eye className="size-5" />}
              </button>
            </div>
          </div>

          {/* 错误提示 */}
          {error && (
            <div className="rounded-lg border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
              {error}
            </div>
          )}

          {/* 创建按钮 */}
          <Button
            onClick={handleCreate}
            disabled={loading}
            className="h-11 w-full rounded-[10px] text-[15px] font-semibold"
          >
            {loading ? '创建中...' : '创建密码'}
          </Button>

          {/* 密码要求 */}
          <div className="space-y-2 text-sm">
            <p className="font-medium text-foreground">密码要求：</p>
            <ul className="space-y-1">
              {requirements.map((req) => (
                <li
                  key={req.label}
                  className={cn(
                    'flex items-center gap-2',
                    req.met ? 'text-primary' : 'text-muted-foreground',
                  )}
                >
                  <Check className={cn('size-3.5', req.met ? 'opacity-100' : 'opacity-30')} />
                  {req.label}
                </li>
              ))}
            </ul>
          </div>
        </div>
      </div>
    </div>
  );
}
