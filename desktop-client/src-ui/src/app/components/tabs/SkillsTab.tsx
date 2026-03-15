import { useState, useEffect } from 'react';
import { Download, Trash2, RefreshCw, Shield, ShieldCheck, ShieldAlert } from 'lucide-react';
import { useTheme } from '../../contexts/ThemeContext';
import { pluginApi } from '../../utils/tauri';

interface Skill {
  id: string;
  name: string;
  version: string;
  description: string;
  trust: 'high' | 'medium' | 'low';
  source: string;
  keywords: string[];
  installed: boolean;
}

export function SkillsTab() {
  const { theme } = useTheme();
  const [activeSubTab, setActiveSubTab] = useState<'available' | 'installed'>('available');
  const [showUninstallConfirm, setShowUninstallConfirm] = useState<string | null>(null);
  const [skills, setSkills] = useState<Skill[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const fetchSkills = async () => {
      try {
        setLoading(true);
        // Fetch installed and available plugins from backend
        const [installed, available] = await Promise.all([
          pluginApi.getInstalledPlugins(),
          pluginApi.getAvailablePlugins(),
        ]);

        // Convert plugins to skills format
        const installedIds = new Set(installed.map((p: any) => p.id));
        const skillsList: Skill[] = available.map((plugin: any, index: number) => ({
          id: plugin.id || String(index),
          name: plugin.name || 'Unknown',
          version: plugin.version || '1.0.0',
          description: plugin.description || 'No description',
          trust: plugin.trust || 'medium',
          source: plugin.author === 'official' ? 'official' : 'community',
          keywords: plugin.keywords || [],
          installed: installedIds.has(plugin.id),
        }));

        setSkills(skillsList);
        setError(null);
      } catch (err) {
        console.error('Failed to fetch skills:', err);
        setError('Failed to load skills');
        // Fallback to sample data
        setSkills([
          {
            id: '1',
            name: '代码审查',
            version: '2.1.0',
            description: '自动审查代码质量、安全性和最佳实践',
            trust: 'high',
            source: 'official',
            keywords: ['代码', '审查', '安全'],
            installed: true,
          },
          {
            id: '2',
            name: '文档生成',
            version: '1.5.0',
            description: '根据代码自动生成技术文档和API说明',
            trust: 'high',
            source: 'official',
            keywords: ['文档', 'API', '生成'],
            installed: true,
          },
        ]);
      } finally {
        setLoading(false);
      }
    };

    fetchSkills();
  }, []);

  const handleInstall = async (id: string) => {
    try {
      await pluginApi.installPlugin(id);
      setSkills(skills.map(skill =>
        skill.id === id ? { ...skill, installed: true } : skill
      ));
    } catch (err) {
      console.error('Failed to install skill:', err);
      alert('Failed to install skill');
    }
  };

  const handleUninstall = async (id: string) => {
    try {
      await pluginApi.uninstallPlugin(id);
      setSkills(skills.map(skill =>
        skill.id === id ? { ...skill, installed: false } : skill
      ));
      setShowUninstallConfirm(null);
    } catch (err) {
      console.error('Failed to uninstall skill:', err);
      alert('Failed to uninstall skill');
    }
  };

  const getTrustIcon = (trust: string) => {
    switch (trust) {
      case 'high':
        return <ShieldCheck className="text-green-400" size={20} />;
      case 'medium':
        return <Shield className="text-yellow-400" size={20} />;
      case 'low':
        return <ShieldAlert className="text-red-400" size={20} />;
      default:
        return <Shield className="text-gray-400" size={20} />;
    }
  };

  const getTrustLabel = (trust: string) => {
    switch (trust) {
      case 'high':
        return '高信任';
      case 'medium':
        return '中等信任';
      case 'low':
        return '低信任';
      default:
        return '未知';
    }
  };

  const installedSkills = skills.filter(skill => skill.installed);
  const availableSkills = skills;

  if (loading) {
    return (
      <div className="h-full flex items-center justify-center">
        <div className={`text-center ${theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}`}>
          <div className="animate-spin mb-4">
            <Shield size={48} className="mx-auto opacity-50" />
          </div>
          <p>加载技能中...</p>
        </div>
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col">
      {/* Sub-tabs */}
      <div className={`border-b ${
        theme === 'dark' ? 'bg-[#0f1d35] border-[#1a2942]' : 'bg-white border-[#ddd]'
      }`}>
        <div className="flex gap-4 px-6">
          <button
            onClick={() => setActiveSubTab('available')}
            className={`px-4 py-3 border-b-2 transition-colors ${
              activeSubTab === 'available'
                ? theme === 'dark' ? 'border-[#5ddad5] text-white font-medium' : 'border-[#667eea] text-[#667eea] font-medium'
                : theme === 'dark' ? 'border-transparent text-gray-400 hover:text-white' : 'border-transparent text-[#666] hover:text-[#667eea]'
            }`}
          >
            可用技能
          </button>
          <button
            onClick={() => setActiveSubTab('installed')}
            className={`px-4 py-3 border-b-2 transition-colors ${
              activeSubTab === 'installed'
                ? theme === 'dark' ? 'border-[#5ddad5] text-white font-medium' : 'border-[#667eea] text-[#667eea] font-medium'
                : theme === 'dark' ? 'border-transparent text-gray-400 hover:text-white' : 'border-transparent text-[#666] hover:text-[#667eea]'
            }`}
          >
            已安装 ({installedSkills.length})
          </button>
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto p-6">
        {error && (
          <div className="mb-4 p-3 bg-red-400/10 border border-red-400/30 rounded-lg text-red-400 text-sm">
            {error}
          </div>
        )}
        {activeSubTab === 'available' && (
          <div className="mb-6 flex gap-3">
            <input
              type="text"
              placeholder="搜索技能..."
              className={`flex-1 px-4 py-2 border rounded-lg focus:outline-none ${
                theme === 'dark'
                  ? 'bg-[#0f1d35] border-[#1a2942] focus:border-[#5ddad5] text-white placeholder-gray-500'
                  : 'bg-white border-[#ddd] focus:border-[#667eea] focus:ring-2 focus:ring-[#667eea]/20 text-[#333] placeholder-gray-400'
              }`}
            />
            <button className={`px-4 py-2 rounded-lg flex items-center gap-2 font-medium transition-opacity ${
              theme === 'dark'
                ? 'bg-gradient-to-r from-[#5ddad5] to-[#4facf7] text-[#0a1628] hover:opacity-90'
                : 'bg-[#667eea] text-white hover:opacity-90 shadow-md'
            }`}>
              <RefreshCw size={18} />
              刷新
            </button>
          </div>
        )}

        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {(activeSubTab === 'installed' ? installedSkills : availableSkills).map((skill) => (
            <div
              key={skill.id}
              className={`border rounded-xl p-5 transition-colors ${
                theme === 'dark'
                  ? 'bg-[#0f1d35] border-[#1a2942] hover:border-[#5ddad5]/30'
                  : 'bg-white border-[#ddd] hover:border-[#667eea]/50 shadow-sm hover:shadow-md'
              }`}
            >
              <div className="flex items-start justify-between mb-3">
                <div>
                  <h3 className={`font-semibold text-lg ${theme === 'dark' ? 'text-white' : 'text-[#333]'}`}>{skill.name}</h3>
                  <p className={`text-sm ${theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}`}>v{skill.version}</p>
                </div>
                {skill.installed && (
                  <span className="px-2 py-1 bg-green-400/10 text-green-400 text-xs rounded-full border border-green-400/30">
                    已安装
                  </span>
                )}
              </div>

              <p className={`text-sm mb-3 ${theme === 'dark' ? 'text-gray-400' : 'text-[#666]'}`}>{skill.description}</p>

              <div className="flex items-center gap-2 mb-3">
                {getTrustIcon(skill.trust)}
                <span className={`text-sm font-medium ${theme === 'dark' ? 'text-white' : 'text-[#333]'}`}>
                  {getTrustLabel(skill.trust)}
                </span>
              </div>

              <div className={`text-xs mb-4 ${theme === 'dark' ? 'text-gray-400' : 'text-[#999]'}`}>
                来源: {skill.source === 'official' ? '官方' : '社区'}
              </div>

              <div className="mb-4">
                <div className="flex flex-wrap gap-1">
                  {skill.keywords.map((keyword, index) => (
                    <span
                      key={index}
                      className={`px-2 py-1 text-xs rounded border ${
                        theme === 'dark'
                          ? 'bg-[#0a1628] text-gray-400 border-[#1a2942]'
                          : 'bg-[#f5f5f5] text-[#666] border-[#eee]'
                      }`}
                    >
                      {keyword}
                    </span>
                  ))}
                </div>
              </div>

              {skill.installed ? (
                <button
                  onClick={() => setShowUninstallConfirm(skill.id)}
                  className="w-full px-4 py-2 bg-red-400/10 hover:bg-red-400/20 text-red-400 rounded-lg flex items-center justify-center gap-2 transition-colors border border-red-400/30"
                >
                  <Trash2 size={16} />
                  卸载
                </button>
              ) : (
                <button
                  onClick={() => handleInstall(skill.id)}
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

      {/* Uninstall Confirmation Modal */}
      {showUninstallConfirm && (
        <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
          <div className={`border rounded-xl p-6 max-w-md w-full mx-4 ${
            theme === 'dark'
              ? 'bg-[#0f1d35] border-[#1a2942]'
              : 'bg-white border-[#ddd] shadow-lg'
          }`}>
            <h3 className={`text-lg font-semibold mb-3 ${theme === 'dark' ? 'text-white' : 'text-[#333]'}`}>
              确认卸载
            </h3>
            <p className={`mb-6 ${theme === 'dark' ? 'text-gray-400' : 'text-[#666]'}`}>
              您确定要卸载这个技能吗？此操作无法撤销。
            </p>
            <div className="flex gap-3 justify-end">
              <button
                onClick={() => setShowUninstallConfirm(null)}
                className={`px-4 py-2 rounded-lg transition-opacity border ${
                  theme === 'dark'
                    ? 'bg-[#0a1628] hover:opacity-80 text-gray-400 border-[#1a2942]'
                    : 'bg-[#f5f5f5] hover:opacity-80 text-[#666] border-[#ddd]'
                }`}
              >
                取消
              </button>
              <button
                onClick={() => handleUninstall(showUninstallConfirm)}
                className="px-4 py-2 bg-gradient-to-r from-red-400 to-red-500 hover:opacity-90 text-white rounded-lg transition-opacity"
              >
                确认卸载
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
