import { useState } from 'react';
import { useNavigate } from 'react-router';
import { Eye, EyeOff } from 'lucide-react';
import { useTheme } from '../../contexts/ThemeContext';
import { authApi } from '../../utils/tauri';

export function PasswordSetup() {
  const navigate = useNavigate();
  const { theme } = useTheme();
  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [showPassword, setShowPassword] = useState(false);
  const [showConfirmPassword, setShowConfirmPassword] = useState(false);
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);

  const getPasswordStrength = (pwd: string) => {
    if (pwd.length < 8) return '弱';
    const hasUpper = /[A-Z]/.test(pwd);
    const hasLower = /[a-z]/.test(pwd);
    const hasNumber = /[0-9]/.test(pwd);
    
    if (hasUpper && hasLower && hasNumber && pwd.length >= 12) return '强';
    if ((hasUpper || hasLower) && hasNumber && pwd.length >= 8) return '中等';
    return '弱';
  };

  const handleCreate = async () => {
    setError('');
    setLoading(true);
    
    if (password.length < 12) {
      setError('密码必须至少12个字符');
      setLoading(false);
      return;
    }
    
    const hasUpper = /[A-Z]/.test(password);
    const hasLower = /[a-z]/.test(password);
    const hasNumber = /[0-9]/.test(password);
    
    if (!hasUpper || !hasLower || !hasNumber) {
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

  const strength = getPasswordStrength(password);
  const strengthColor = strength === '强' 
    ? theme === 'dark' ? 'from-green-400 to-emerald-400' : 'from-green-500 to-emerald-500'
    : strength === '中等' 
    ? theme === 'dark' ? 'from-yellow-400 to-orange-400' : 'from-yellow-500 to-orange-500'
    : theme === 'dark' ? 'from-red-400 to-pink-400' : 'from-red-500 to-pink-500';

  return (
    <div className={`min-h-screen flex items-center justify-center ${
      theme === 'dark' ? 'bg-[#0a1628]' : 'bg-[#f5f5f5]'
    }`}>
      <div className={`p-8 rounded-2xl w-full max-w-md ${
        theme === 'dark'
          ? 'bg-[#0f1d35] border border-[#1a2942]'
          : 'bg-white border border-[#ddd] shadow-lg'
      }`}>
        <div className="text-center mb-8">
          <h1 className={`text-4xl font-bold mb-2 ${
            theme === 'dark'
              ? 'bg-gradient-to-r from-[#5ddad5] to-[#4facf7] bg-clip-text text-transparent'
              : 'text-[#667eea]'
          }`}>
            IronClaw
          </h1>
          <p className={theme === 'dark' ? 'text-gray-400' : 'text-[#666]'}>创建您的主密码</p>
        </div>

        <div className="space-y-6">
          <div>
            <label className={`block text-sm font-medium mb-2 ${
              theme === 'dark' ? 'text-white' : 'text-[#333]'
            }`}>
              主密码
            </label>
            <div className="relative">
              <input
                type={showPassword ? 'text' : 'password'}
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                className={`w-full px-4 py-3 border rounded-lg focus:outline-none transition-colors pr-12 ${
                  theme === 'dark'
                    ? 'bg-[#0a1628] border-[#1a2942] focus:border-[#5ddad5] text-white placeholder-gray-500'
                    : 'bg-white border-[#ddd] focus:border-[#667eea] focus:ring-2 focus:ring-[#667eea]/20 text-[#333] placeholder-gray-400'
                }`}
                placeholder="至少12个字符"
              />
              <button
                type="button"
                onClick={() => setShowPassword(!showPassword)}
                className={`absolute right-3 top-1/2 -translate-y-1/2 transition-colors ${
                  theme === 'dark'
                    ? 'text-gray-400 hover:text-[#5ddad5]'
                    : 'text-gray-400 hover:text-[#667eea]'
                }`}
              >
                {showPassword ? <EyeOff size={20} /> : <Eye size={20} />}
              </button>
            </div>
            {password && (
              <div className="mt-2 flex items-center gap-2">
                <div className={`flex-1 h-2 rounded-full overflow-hidden ${
                  theme === 'dark' ? 'bg-[#0a1628]' : 'bg-gray-200'
                }`}>
                  <div 
                    className={`h-full bg-gradient-to-r ${strengthColor} transition-all duration-300`}
                    style={{ width: strength === '强' ? '100%' : strength === '中等' ? '66%' : '33%' }}
                  />
                </div>
                <span className={`text-sm font-medium ${theme === 'dark' ? 'text-gray-400' : 'text-[#666]'}`}>{strength}</span>
              </div>
            )}
          </div>

          <div>
            <label className={`block text-sm font-medium mb-2 ${
              theme === 'dark' ? 'text-white' : 'text-[#333]'
            }`}>
              确认密码
            </label>
            <div className="relative">
              <input
                type={showConfirmPassword ? 'text' : 'password'}
                value={confirmPassword}
                onChange={(e) => setConfirmPassword(e.target.value)}
                className={`w-full px-4 py-3 border rounded-lg focus:outline-none transition-colors pr-12 ${
                  theme === 'dark'
                    ? 'bg-[#0a1628] border-[#1a2942] focus:border-[#5ddad5] text-white placeholder-gray-500'
                    : 'bg-white border-[#ddd] focus:border-[#667eea] focus:ring-2 focus:ring-[#667eea]/20 text-[#333] placeholder-gray-400'
                }`}
                placeholder="再次输入密码"
              />
              <button
                type="button"
                onClick={() => setShowConfirmPassword(!showConfirmPassword)}
                className={`absolute right-3 top-1/2 -translate-y-1/2 transition-colors ${
                  theme === 'dark'
                    ? 'text-gray-400 hover:text-[#5ddad5]'
                    : 'text-gray-400 hover:text-[#667eea]'
                }`}
              >
                {showConfirmPassword ? <EyeOff size={20} /> : <Eye size={20} />}
              </button>
            </div>
          </div>

          {error && (
            <div className={`px-4 py-3 rounded-lg ${
              theme === 'dark'
                ? 'bg-red-400/10 border border-red-400/30 text-red-400'
                : 'bg-red-50 border border-red-200 text-red-600'
            }`}>
              {error}
            </div>
          )}

          <button
            onClick={handleCreate}
            disabled={loading}
            className={`w-full font-medium py-3 rounded-lg transition-opacity ${
              theme === 'dark'
                ? 'bg-gradient-to-r from-[#5ddad5] to-[#4facf7] text-[#0a1628] hover:opacity-90 disabled:opacity-50'
                : 'bg-[#667eea] text-white hover:opacity-90 shadow-md hover:shadow-lg disabled:opacity-50'
            }`}
          >
            {loading ? '创建中...' : '创建密码'}
          </button>
        </div>

        <div className="mt-6 text-sm">
          <p className={theme === 'dark' ? 'text-white' : 'text-[#333]'}>密码要求：</p>
          <ul className={`space-y-1 mt-2 ${theme === 'dark' ? 'text-gray-400' : 'text-[#666]'}`}>
            <li className={password.length >= 12 ? 'text-green-500' : ''}>• 至少12个字符</li>
            <li className={/[A-Z]/.test(password) ? 'text-green-500' : ''}>• 包含大写字母</li>
            <li className={/[a-z]/.test(password) ? 'text-green-500' : ''}>• 包含小写字母</li>
            <li className={/[0-9]/.test(password) ? 'text-green-500' : ''}>• 包含数字</li>
          </ul>
        </div>
      </div>
    </div>
  );
}
