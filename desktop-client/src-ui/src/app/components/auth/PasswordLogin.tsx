import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router';
import { Eye, EyeOff, Lock } from 'lucide-react';
import { useTheme } from '../../contexts/ThemeContext';
import { authApi } from '../../utils/tauri';

export function PasswordLogin() {
  const navigate = useNavigate();
  const { theme } = useTheme();
  const [password, setPassword] = useState('');
  const [showPassword, setShowPassword] = useState(false);
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    // Check if already authenticated
    const sessionId = sessionStorage.getItem('session_id');
    if (sessionId) {
      navigate('/app');
      return;
    }

    // Check setup status — 嵌入式模式下自动跳转
    authApi.checkSetupStatus().then(async (status) => {
      if (!status.password_set) {
        navigate('/setup');
        return;
      }
      // 嵌入式模式：authApi.unlockApp 返回 stub 成功，自动解锁
      try {
        const result = await authApi.unlockApp('');
        if (result.success && result.session_id) {
          sessionStorage.setItem('session_id', result.session_id);
          navigate('/app');
        }
      } catch {
        // 非嵌入式模式：保持密码输入页面
      }
    }).catch(err => {
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

  const handleKeyPress = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      handleUnlock();
    }
  };

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
          <div className={`inline-flex items-center justify-center w-20 h-20 rounded-2xl mb-4 ${
            theme === 'dark'
              ? 'bg-gradient-to-br from-[#5ddad5]/20 to-[#4facf7]/20'
              : 'bg-[#667eea]/10'
          }`}>
            <Lock className={theme === 'dark' ? 'text-white' : 'text-[#667eea]'} size={40} />
          </div>
          <h1 className={`text-4xl font-bold mb-2 ${
            theme === 'dark'
              ? 'bg-gradient-to-r from-[#5ddad5] to-[#4facf7] bg-clip-text text-transparent'
              : 'text-[#667eea]'
          }`}>
            IronClaw
          </h1>
          <p className={theme === 'dark' ? 'text-gray-400' : 'text-[#666]'}>您的安全AI助手</p>
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
                onKeyPress={handleKeyPress}
                className={`w-full px-4 py-3 border rounded-lg focus:outline-none transition-colors pr-12 ${
                  theme === 'dark'
                    ? 'bg-[#0a1628] border-[#1a2942] focus:border-[#5ddad5] text-white placeholder-gray-500'
                    : 'bg-white border-[#ddd] focus:border-[#667eea] focus:ring-2 focus:ring-[#667eea]/20 text-[#333] placeholder-gray-400'
                }`}
                placeholder="输入您的主密码"
                autoFocus
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
            onClick={handleUnlock}
            disabled={loading}
            className={`w-full font-medium py-3 rounded-lg transition-opacity ${
              theme === 'dark'
                ? 'bg-gradient-to-r from-[#5ddad5] to-[#4facf7] text-[#0a1628] hover:opacity-90 disabled:opacity-50'
                : 'bg-[#667eea] text-white hover:opacity-90 shadow-md hover:shadow-lg disabled:opacity-50'
            }`}
          >
            {loading ? '解锁中...' : '解锁'}
          </button>

          <div className="text-center">
            <button
              onClick={() => navigate('/setup')}
              className={`text-sm transition-opacity hover:opacity-80 ${
                theme === 'dark'
                  ? 'bg-gradient-to-r from-[#5ddad5] to-[#4facf7] bg-clip-text text-transparent'
                  : 'text-[#667eea]'
              }`}
            >
              首次使用？设置密码
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}