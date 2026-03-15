import { useState, useEffect } from 'react';
import { Download, Trash2, Power, PowerOff, RefreshCw } from 'lucide-react';
import { useTheme } from '../../contexts/ThemeContext';
import { extensionApi, type ExtensionMetadata, type InstalledExtension } from '../../utils/tauri';

export function ExtensionsTab() {
  const { theme } = useTheme();
  const [activeSubTab, setActiveSubTab] = useState<'installed' | 'available'>('installed');
  const [installedExtensions, setInstalledExtensions] = useState<InstalledExtension[]>([]);
  const [availableExtensions, setAvailableExtensions] = useState<ExtensionMetadata[]>([]);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    loadExtensions();
  }, []);

  const loadExtensions = async () => {
    setLoading(true);
    try {
      const [installed, available] = await Promise.all([
        extensionApi.getInstalledExtensions(),
        extensionApi.getAvailableExtensions(),
      ]);
      setInstalledExtensions(installed);
      setAvailableExtensions(available);
    } catch (err) {
      console.error('Failed to load extensions:', err);
    } finally {
      setLoading(false);
    }
  };

  const handleInstall = async (metadata: ExtensionMetadata) => {
    try {
      await extensionApi.installExtension(metadata);
      await loadExtensions();
    } catch (err) {
      console.error('Failed to install extension:', err);
    }
  };

  const handleUninstall = async (extensionId: string) => {
    if (confirm('确定要卸载此扩展吗？')) {
      try {
        await extensionApi.uninstallExtension(extensionId);
        await loadExtensions();
      } catch (err) {
        console.error('Failed to uninstall extension:', err);
      }
    }
  };

  const handleToggle = async (extensionId: string, currentlyEnabled: boolean) => {
    try {
      if (currentlyEnabled) {
        await extensionApi.disableExtension(extensionId);
      } else {
        await extensionApi.enableExtension(extensionId);
      }
      await loadExtensions();
    } catch (err) {
      console.error('Failed to toggle extension:', err);
    }
  };

  return (
    <div className="h-full flex flex-col">
      {/* Sub-tabs */}
      <div className={`border-b ${
        theme === 'dark' ? 'bg-[#0f1d35] border-[#1a2942]' : 'bg-white border-[#ddd]'
      }`}>
        <div className="flex gap-4 px-6">
          <button
            onClick={() => setActiveSubTab('installed')}
            className={`px-4 py-3 border-b-2 transition-colors ${
              activeSubTab === 'installed'
                ? theme === 'dark' ? 'border-[#5ddad5] text-white font-medium' : 'border-[#667eea] text-[#667eea] font-medium'
                : theme === 'dark' ? 'border-transparent text-gray-400 hover:text-white' : 'border-transparent text-[#666] hover:text-[#667eea]'
            }`}
          >
            已安装 ({installedExtensions.length})
          </button>
          <button
            onClick={() => setActiveSubTab('available')}
            className={`px-4 py-3 border-b-2 transition-colors ${
              activeSubTab === 'available'
                ? theme === 'dark' ? 'border-[#5ddad5] text-white font-medium' : 'border-[#667eea] text-[#667eea] font-medium'
                : theme === 'dark' ? 'border-transparent text-gray-400 hover:text-white' : 'border-transparent text-[#666] hover:text-[#667eea]'
            }`}
          >
            可用扩展
          </button>
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto p-6">
        {activeSubTab === 'available' && (
          <div className="mb-6 flex gap-3">
            <input
              type="text"
              placeholder="搜索扩展..."
              className={`flex-1 px-4 py-2 border rounded-lg focus:outline-none ${
                theme === 'dark'
                  ? 'bg-[#0f1d35] border-[#1a2942] focus:border-[#5ddad5] text-white placeholder-gray-500'
                  : 'bg-white border-[#ddd] focus:border-[#667eea] focus:ring-2 focus:ring-[#667eea]/20 text-[#333] placeholder-gray-400'
              }`}
            />
            <button 
              onClick={loadExtensions}
              disabled={loading}
              className={`px-4 py-2 rounded-lg flex items-center gap-2 font-medium transition-opacity ${
              theme === 'dark'
                ? 'bg-gradient-to-r from-[#5ddad5] to-[#4facf7] text-[#0a1628] hover:opacity-90 disabled:opacity-50'
                : 'bg-[#667eea] text-white hover:opacity-90 shadow-md disabled:opacity-50'
            }`}>
              <RefreshCw size={18} className={loading ? 'animate-spin' : ''} />
              刷新
            </button>
          </div>
        )}

        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {(activeSubTab === 'installed' 
            ? installedExtensions.map(ext => ({ ...ext.metadata, installed: true, enabled: ext.enabled }))
            : availableExtensions.map(ext => ({ ...ext, installed: false, enabled: false }))
          ).map((ext) => (
            <div
              key={ext.id}
              className={`border rounded-xl p-5 transition-colors ${
                theme === 'dark'
                  ? 'bg-[#0f1d35] border-[#1a2942] hover:border-[#5ddad5]/30'
                  : 'bg-white border-[#ddd] hover:border-[#667eea]/50 shadow-sm hover:shadow-md'
              }`}
            >
              <div className="flex items-start justify-between mb-3">
                <div>
                  <h3 className={`font-semibold text-lg ${theme === 'dark' ? 'text-white' : 'text-[#333]'}`}>{ext.name}</h3>
                  <p className={`text-sm ${theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}`}>v{ext.version}</p>
                </div>
                {ext.installed && ext.enabled && (
                  <span className="px-2 py-1 bg-green-400/10 text-green-400 text-xs rounded-full border border-green-400/30">
                    已启用
                  </span>
                )}
                {ext.installed && !ext.enabled && (
                  <span className="px-2 py-1 bg-gray-400/10 text-gray-400 text-xs rounded-full border border-gray-400/30">
                    已禁用
                  </span>
                )}
              </div>

              <p className={`text-sm mb-3 ${theme === 'dark' ? 'text-gray-400' : 'text-[#666]'}`}>{ext.description}</p>

              <div className={`text-xs mb-3 ${theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}`}>
                作者: {ext.author}
              </div>

              <div className="mb-4">
                <div className={`text-xs font-medium mb-2 ${theme === 'dark' ? 'text-white' : 'text-[#333]'}`}>提供的工具:</div>
                <div className="flex flex-wrap gap-1">
                  {ext.tools.map((tool, index) => (
                    <span
                      key={index}
                      className={`px-2 py-1 text-xs rounded border ${
                        theme === 'dark'
                          ? 'bg-[#0a1628] text-gray-400 border-[#1a2942]'
                          : 'bg-[#f5f5f5] text-[#666] border-[#eee]'
                      }`}
                    >
                      {tool}
                    </span>
                  ))}
                </div>
              </div>

              {ext.installed ? (
                <div className="flex gap-2">
                  <button
                    onClick={() => handleToggle(ext.id, ext.enabled || false)}
                    className={`flex-1 px-4 py-2 rounded-lg flex items-center justify-center gap-2 font-medium transition-opacity ${
                      ext.enabled
                        ? theme === 'dark'
                          ? 'bg-[#0a1628] hover:opacity-80 text-gray-400 border border-[#1a2942]'
                          : 'bg-[#f5f5f5] hover:opacity-80 text-[#666] border border-[#ddd]'
                        : theme === 'dark'
                          ? 'bg-gradient-to-r from-[#5ddad5] to-[#4facf7] hover:opacity-90 text-[#0a1628]'
                          : 'bg-[#667eea] hover:opacity-90 text-white shadow-md'
                    }`}
                  >
                    {ext.enabled ? <PowerOff size={16} /> : <Power size={16} />}
                    {ext.enabled ? '禁用' : '启用'}
                  </button>
                  <button
                    onClick={() => handleUninstall(ext.id)}
                    className="px-4 py-2 bg-red-400/10 hover:bg-red-400/20 text-red-400 rounded-lg flex items-center gap-2 transition-colors border border-red-400/30"
                  >
                    <Trash2 size={16} />
                    卸载
                  </button>
                </div>
              ) : (
                <button
                  onClick={() => handleInstall(ext as ExtensionMetadata)}
                  className={`w-full px-4 py-2 rounded-lg flex items-center justify-center gap-2 font-medium transition-opacity ${
                    theme === 'dark'
                      ? 'bg-gradient-to-r from-[#5ddad5] to-[#4facf7] text-[#0a1628] hover:opacity-90'
                      : 'bg-[#667eea] text-white hover:opacity-90 shadow-md'
                  }`}
                >
                  <Download size={16} />
                  安装
                </button>
              )}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
