import { useState, useEffect } from 'react';
import { Save, RotateCcw } from 'lucide-react';
import { useTheme } from '../../contexts/ThemeContext';
import { configApi } from '../../utils/tauri';

export function SettingsTab() {
  const { theme } = useTheme();
  const [apiUrl, setApiUrl] = useState('http://localhost:3000');
  const [logLevel, setLogLevel] = useState('info');
  const [autoSave, setAutoSave] = useState(true);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null);

  useEffect(() => {
    loadSettings();
  }, []);

  const loadSettings = async () => {
    try {
      const url = await configApi.getConfig('api_url');
      if (url) setApiUrl(url);
      
      const level = await configApi.getConfig('log_level');
      if (level) setLogLevel(level);
      
      const auto = await configApi.getConfig('auto_save');
      if (auto) setAutoSave(auto === 'true');
    } catch (err) {
      console.error('Failed to load settings:', err);
    }
  };

  const handleSave = async () => {
    setSaving(true);
    try {
      await configApi.storeConfig('api_url', apiUrl);
      await configApi.storeConfig('log_level', logLevel);
      await configApi.storeConfig('auto_save', autoSave.toString());
      
      setMessage({ type: 'success', text: '设置已保存' });
      setTimeout(() => setMessage(null), 3000);
    } catch (err) {
      console.error('Failed to save settings:', err);
      setMessage({ type: 'error', text: '保存设置失败' });
    } finally {
      setSaving(false);
    }
  };

  const handleReset = async () => {
    if (confirm('确定要重置所有设置吗？')) {
      setApiUrl('http://localhost:3000');
      setLogLevel('info');
      setAutoSave(true);
      await handleSave();
    }
  };

  return (
    <div className={`flex-1 overflow-y-auto p-6 ${
      theme === 'dark' ? 'bg-[#0a1628]' : 'bg-[#f5f5f5]'
    }`}>
      <div className="max-w-2xl mx-auto space-y-6">
        {/* Header */}
        <div className="mb-8">
          <h1 className={`text-3xl font-bold mb-2 ${
            theme === 'dark' ? 'text-white' : 'text-[#333]'
          }`}>
            设置
          </h1>
          <p className={`text-sm ${
            theme === 'dark' ? 'text-gray-400' : 'text-[#666]'
          }`}>
            配置应用程序的行为和连接参数
          </p>
        </div>

        {/* Message */}
        {message && (
          <div className={`mb-6 p-4 rounded-lg ${
            message.type === 'success'
              ? theme === 'dark'
                ? 'bg-green-400/10 border border-green-400/30 text-green-400'
                : 'bg-green-50 border border-green-200 text-green-700'
              : theme === 'dark'
              ? 'bg-red-400/10 border border-red-400/30 text-red-400'
              : 'bg-red-50 border border-red-200 text-red-700'
          }`}>
            {message.text}
          </div>
        )}

        {/* Settings Sections */}
        <div className="space-y-6">
          {/* API Configuration */}
          <div className={`p-6 rounded-lg border ${
            theme === 'dark'
              ? 'bg-[#0f1d35] border-[#1a2942]'
              : 'bg-white border-[#ddd]'
          }`}>
            <h2 className={`text-lg font-semibold mb-4 ${
              theme === 'dark' ? 'text-white' : 'text-[#333]'
            }`}>
              API 配置
            </h2>
            
            <div className="space-y-4">
              <div>
                <label className={`block text-sm font-medium mb-2 ${
                  theme === 'dark' ? 'text-gray-300' : 'text-[#666]'
                }`}>
                  API 基础 URL
                </label>
                <input
                  type="text"
                  value={apiUrl}
                  onChange={(e) => setApiUrl(e.target.value)}
                  placeholder="http://localhost:3000"
                  className={`w-full px-4 py-2 border rounded-lg focus:outline-none ${
                    theme === 'dark'
                      ? 'bg-[#0a1628] border-[#1a2942] focus:border-[#5ddad5] text-white'
                      : 'bg-white border-[#ddd] focus:border-[#667eea] focus:ring-2 focus:ring-[#667eea]/20 text-[#333]'
                  }`}
                />
                <p className={`text-xs mt-2 ${
                  theme === 'dark' ? 'text-gray-500' : 'text-[#999]'
                }`}>
                  后端服务的地址。默认为 http://localhost:3000
                </p>
              </div>
            </div>
          </div>

          {/* Logging Configuration */}
          <div className={`p-6 rounded-lg border ${
            theme === 'dark'
              ? 'bg-[#0f1d35] border-[#1a2942]'
              : 'bg-white border-[#ddd]'
          }`}>
            <h2 className={`text-lg font-semibold mb-4 ${
              theme === 'dark' ? 'text-white' : 'text-[#333]'
            }`}>
              日志配置
            </h2>
            
            <div className="space-y-4">
              <div>
                <label className={`block text-sm font-medium mb-2 ${
                  theme === 'dark' ? 'text-gray-300' : 'text-[#666]'
                }`}>
                  日志级别
                </label>
                <select
                  value={logLevel}
                  onChange={(e) => setLogLevel(e.target.value)}
                  className={`w-full px-4 py-2 border rounded-lg focus:outline-none ${
                    theme === 'dark'
                      ? 'bg-[#0a1628] border-[#1a2942] focus:border-[#5ddad5] text-white'
                      : 'bg-white border-[#ddd] focus:border-[#667eea] focus:ring-2 focus:ring-[#667eea]/20 text-[#333]'
                  }`}
                >
                  <option value="debug">调试 (Debug)</option>
                  <option value="info">信息 (Info)</option>
                  <option value="warn">警告 (Warn)</option>
                  <option value="error">错误 (Error)</option>
                </select>
                <p className={`text-xs mt-2 ${
                  theme === 'dark' ? 'text-gray-500' : 'text-[#999]'
                }`}>
                  控制日志的详细程度。调试模式会显示更多信息。
                </p>
              </div>
            </div>
          </div>

          {/* General Settings */}
          <div className={`p-6 rounded-lg border ${
            theme === 'dark'
              ? 'bg-[#0f1d35] border-[#1a2942]'
              : 'bg-white border-[#ddd]'
          }`}>
            <h2 className={`text-lg font-semibold mb-4 ${
              theme === 'dark' ? 'text-white' : 'text-[#333]'
            }`}>
              常规设置
            </h2>
            
            <div className="space-y-4">
              <label className={`flex items-center gap-3 cursor-pointer ${
                theme === 'dark' ? 'text-gray-300' : 'text-[#666]'
              }`}>
                <input
                  type="checkbox"
                  checked={autoSave}
                  onChange={(e) => setAutoSave(e.target.checked)}
                  className="w-4 h-4 rounded"
                />
                <span className="text-sm font-medium">自动保存设置</span>
              </label>
              <p className={`text-xs ${
                theme === 'dark' ? 'text-gray-500' : 'text-[#999]'
              }`}>
                启用后，设置更改会自动保存
              </p>
            </div>
          </div>

          {/* About */}
          <div className={`p-6 rounded-lg border ${
            theme === 'dark'
              ? 'bg-[#0f1d35] border-[#1a2942]'
              : 'bg-white border-[#ddd]'
          }`}>
            <h2 className={`text-lg font-semibold mb-4 ${
              theme === 'dark' ? 'text-white' : 'text-[#333]'
            }`}>
              关于
            </h2>
            
            <div className="space-y-2">
              <p className={`text-sm ${
                theme === 'dark' ? 'text-gray-400' : 'text-[#666]'
              }`}>
                <span className="font-medium">应用名称：</span> IronClaw 桌面客户端
              </p>
              <p className={`text-sm ${
                theme === 'dark' ? 'text-gray-400' : 'text-[#666]'
              }`}>
                <span className="font-medium">版本：</span> 0.1.0
              </p>
              <p className={`text-sm ${
                theme === 'dark' ? 'text-gray-400' : 'text-[#666]'
              }`}>
                <span className="font-medium">后端 URL：</span> {apiUrl}
              </p>
            </div>
          </div>
        </div>

        {/* Action Buttons */}
        <div className="mt-8 flex gap-4">
          <button
            onClick={handleSave}
            disabled={saving}
            className={`flex items-center gap-2 px-6 py-3 rounded-lg font-medium transition-opacity ${
              theme === 'dark'
                ? 'bg-gradient-to-r from-[#5ddad5] to-[#4facf7] text-[#0a1628] hover:opacity-90 disabled:opacity-50'
                : 'bg-[#667eea] text-white hover:opacity-90 shadow-md disabled:opacity-50'
            }`}
          >
            <Save size={18} />
            <span>{saving ? '保存中...' : '保存设置'}</span>
          </button>
          
          <button
            onClick={handleReset}
            className={`flex items-center gap-2 px-6 py-3 rounded-lg font-medium transition-colors ${
              theme === 'dark'
                ? 'bg-[#0f1d35] border border-[#1a2942] text-gray-300 hover:bg-[#1a2942]'
                : 'bg-white border border-[#ddd] text-[#666] hover:bg-[#f5f5f5]'
            }`}
          >
            <RotateCcw size={18} />
            <span>重置为默认值</span>
          </button>
        </div>
      </div>
    </div>
  );
}
