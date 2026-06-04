/**
 * ExtensionsTab - 扩展管理面板
 *
 * 设计风格与 SkillsTab 一致：搜索栏 + 2列卡片网格
 * 每张卡片：图标 + 名称 + 描述 + 开关 + 标签 + 更多菜单
 */

import { useState, useEffect, useCallback, useRef } from 'react';
import {
  Search,
  Plus,
  Puzzle,
  ShieldCheck,
  MoreHorizontal,
  Info,
  Trash2,
  Settings,
} from 'lucide-react';
import { Switch } from '../ui/switch';
import { cn } from '../ui/utils';
import {
  extensionApi,
  extensionSetupApi,
  type ExtensionMetadata,
  type InstalledExtension,
  type ExtensionSetupField,
  type ExtensionSetupSubmitResponse,
} from '../../utils/tauri';

/* ── 类型定义 ── */

interface ExtCardData {
  id: string;
  name: string;
  description: string;
  version: string;
  author: string;
  enabled: boolean;
  installed: boolean;
  tools: string[];
}

/* ── 主组件 ── */

export function ExtensionsTab() {
  const [extensions, setExtensions] = useState<ExtCardData[]>([]);
  const [searchQuery, setSearchQuery] = useState('');
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [menuOpenId, setMenuOpenId] = useState<string | null>(null);
  const menuRef = useRef<HTMLDivElement>(null);

  // Setup modal state
  const [setupExtName, setSetupExtName] = useState<string | null>(null);
  const [setupFields, setSetupFields] = useState<ExtensionSetupField[]>([]);
  const [setupFieldNames, setSetupFieldNames] = useState<Set<string>>(new Set());
  const [setupValues, setSetupValues] = useState<Record<string, string>>({});
  const [setupLoading, setSetupLoading] = useState(false);
  const [setupResult, setSetupResult] = useState<ExtensionSetupSubmitResponse | null>(null);

  const loadExtensions = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const [installed, available] = await Promise.all([
        extensionApi.getInstalledExtensions(),
        extensionApi.getAvailableExtensions(),
      ]);

      const installedMap = new Map(
        installed.map((e: InstalledExtension) => [e.metadata.id, e]),
      );

      const merged: ExtCardData[] = available.map((e: ExtensionMetadata) => {
        const inst = installedMap.get(e.id);
        return {
          id: e.id,
          name: e.name,
          description: e.description,
          version: e.version,
          author: e.author,
          enabled: inst?.enabled ?? false,
          installed: !!inst,
          tools: e.tools,
        };
      });

      // 补充已安装但不在 available 列表中的
      for (const [id, inst] of installedMap) {
        if (!merged.some((m) => m.id === id)) {
          merged.push({
            id,
            name: inst.metadata.name,
            description: inst.metadata.description,
            version: inst.metadata.version,
            author: inst.metadata.author,
            enabled: inst.enabled,
            installed: true,
            tools: inst.metadata.tools,
          });
        }
      }

      setExtensions(merged);
    } catch (err) {
      console.error('Failed to load extensions:', err);
      setError('加载扩展失败');
      setExtensions([]);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadExtensions();
  }, [loadExtensions]);

  // 点击外部关闭菜单
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setMenuOpenId(null);
      }
    };
    if (menuOpenId) {
      document.addEventListener('mousedown', handleClickOutside);
    }
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [menuOpenId]);

  const handleToggle = async (id: string, currentEnabled: boolean) => {
    setExtensions((prev) =>
      prev.map((e) => (e.id === id ? { ...e, enabled: !currentEnabled } : e)),
    );
    try {
      if (currentEnabled) {
        await extensionApi.disableExtension(id);
      } else {
        await extensionApi.enableExtension(id);
      }
    } catch {
      setExtensions((prev) =>
        prev.map((e) => (e.id === id ? { ...e, enabled: currentEnabled } : e)),
      );
    }
  };

  const handleInstall = async (ext: ExtCardData) => {
    try {
      await extensionApi.installExtension({
        id: ext.id,
        name: ext.name,
        description: ext.description,
        version: ext.version,
        author: ext.author,
        tools: ext.tools,
      } as ExtensionMetadata);
      await loadExtensions();
    } catch (err) {
      console.error('Failed to install extension:', err);
    }
  };

  const handleUninstall = async (id: string) => {
    setMenuOpenId(null);
    try {
      await extensionApi.uninstallExtension(id);
      await loadExtensions();
    } catch (err) {
      console.error('Failed to uninstall extension:', err);
    }
  };

  const handleSetup = async (name: string) => {
    setMenuOpenId(null);
    setSetupExtName(name);
    setSetupResult(null);
    setSetupLoading(true);
    try {
      const schema = await extensionSetupApi.getSetupSchema(name);
      const allFields = [...schema.secrets, ...schema.fields];
      setSetupFields(allFields);
      setSetupFieldNames(new Set(schema.fields.map((field) => field.name)));
      const initial: Record<string, string> = {};
      allFields.forEach((f) => { initial[f.name] = ''; });
      setSetupValues(initial);
    } catch {
      setSetupFields([]);
      setSetupFieldNames(new Set());
    } finally {
      setSetupLoading(false);
    }
  };

  const handleSetupSubmit = async () => {
    if (!setupExtName) return;
    setSetupLoading(true);
    try {
      const secrets: Record<string, string> = {};
      const fields: Record<string, string> = {};
      Object.entries(setupValues).forEach(([name, value]) => {
        if (setupFieldNames.has(name)) {
          fields[name] = value;
        } else {
          secrets[name] = value;
        }
      });
      const result = await extensionSetupApi.submitSetup(setupExtName, secrets, fields);
      setSetupResult(result);
      if (result.activated) await loadExtensions();
      if (result.auth_url) window.open(result.auth_url, '_blank');
    } catch (err) {
      setSetupResult({
        success: false,
        message: String(err),
        activated: false,
        auth_url: null,
      });
    } finally {
      setSetupLoading(false);
    }
  };

  const filtered = extensions.filter(
    (e) =>
      !searchQuery.trim() ||
      e.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
      e.description.toLowerCase().includes(searchQuery.toLowerCase()),
  );

  const rows: ExtCardData[][] = [];
  for (let i = 0; i < filtered.length; i += 2) {
    rows.push(filtered.slice(i, i + 2));
  }

  return (
    <div className="flex flex-col gap-4">
      {error && (
        <div className="rounded-lg border border-destructive/30 bg-destructive/5 px-3 py-2 text-xs text-destructive">
          {error}
        </div>
      )}

      {/* 搜索栏 + 添加按钮 */}
      <div className="flex items-center gap-3">
        <div className="relative flex-1">
          <Search className="absolute left-3.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="搜索已安装的扩展"
            className="h-10 w-full rounded-[10px] border border-border bg-secondary/50 pl-10 pr-3 text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-primary/30"
          />
        </div>
        <button
          onClick={loadExtensions}
          className="flex h-10 items-center gap-1.5 rounded-[10px] bg-primary px-4 text-sm font-semibold text-primary-foreground transition-opacity hover:opacity-90"
        >
          <Plus className="size-4" />
          刷新扩展
        </button>
      </div>

      {/* 扩展卡片网格 */}
      {loading ? (
        <div className="flex items-center justify-center py-16 text-sm text-muted-foreground">
          加载中...
        </div>
      ) : filtered.length === 0 ? (
        <div className="flex items-center justify-center py-16 text-sm text-muted-foreground">
          {searchQuery ? '未找到匹配的扩展' : '暂无扩展'}
        </div>
      ) : (
        <div className="flex flex-col gap-3">
          {rows.map((row, ri) => (
            <div key={ri} className="flex gap-3">
              {row.map((ext) => (
                <ExtCard
                  key={ext.id}
                  ext={ext}
                  menuOpen={menuOpenId === ext.id}
                  onToggle={() => handleToggle(ext.id, ext.enabled)}
                  onMenuToggle={() =>
                    setMenuOpenId(menuOpenId === ext.id ? null : ext.id)
                  }
                  onInstall={() => handleInstall(ext)}
                  onUninstall={() => handleUninstall(ext.id)}
                  onSetup={() => handleSetup(ext.id)}
                  menuRef={menuOpenId === ext.id ? menuRef : undefined}
                />
              ))}
              {row.length === 1 && <div className="flex-1" />}
            </div>
          ))}
        </div>
      )}

      {/* Setup Modal */}
      {setupExtName && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
          <div className="mx-4 w-full max-w-md rounded-2xl border border-border bg-background p-6 shadow-xl">
            <h3 className="mb-4 flex items-center gap-2 text-base font-semibold text-foreground">
              <Settings className="size-4" />
              配置 {setupExtName}
            </h3>

            {setupLoading && setupFields.length === 0 ? (
              <p className="py-4 text-sm text-muted-foreground">加载配置...</p>
            ) : setupFields.length === 0 ? (
              <p className="py-4 text-sm text-muted-foreground">此扩展无需配置</p>
            ) : (
              <div className="space-y-3">
                {setupFields.map((field) => (
                  <div key={field.name}>
                    <label className="mb-1 block text-sm font-medium text-foreground">
                      {field.prompt}
                      {field.optional && (
                        <span className="ml-1 text-xs text-muted-foreground">(可选)</span>
                      )}
                      {field.provided && (
                        <span className="ml-1 text-xs text-primary">✓ 已配置</span>
                      )}
                    </label>
                    <input
                      type={field.input_type === 'Password' ? 'password' : 'text'}
                      value={setupValues[field.name] || ''}
                      onChange={(e) =>
                        setSetupValues((prev) => ({
                          ...prev,
                          [field.name]: e.target.value,
                        }))
                      }
                      placeholder={
                        field.auto_generate
                          ? '留空自动生成'
                          : field.provided
                            ? '留空保持不变'
                            : '请输入...'
                      }
                      className="w-full rounded-lg border border-border bg-secondary/30 px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-primary/30"
                    />
                  </div>
                ))}
              </div>
            )}

            {setupResult && (
              <div
                className={cn(
                  'mt-4 rounded-lg border p-3 text-sm',
                  setupResult.success
                    ? 'border-primary/30 bg-primary/5 text-primary'
                    : 'border-destructive/30 bg-destructive/5 text-destructive',
                )}
              >
                {setupResult.message}
              </div>
            )}

            <div className="mt-6 flex justify-end gap-3">
              <button
                onClick={() => {
                  setSetupExtName(null);
                  setSetupResult(null);
                  setSetupFieldNames(new Set());
                }}
                className="rounded-lg border border-border px-4 py-2 text-sm text-muted-foreground transition-colors hover:bg-accent"
              >
                关闭
              </button>
              {setupFields.length > 0 && (
                <button
                  onClick={handleSetupSubmit}
                  disabled={setupLoading}
                  className="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition-opacity hover:opacity-90 disabled:opacity-50"
                >
                  {setupLoading ? '提交中...' : '保存配置'}
                </button>
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

/* ── 扩展卡片 ── */

function ExtCard({
  ext,
  menuOpen,
  onToggle,
  onMenuToggle,
  onInstall,
  onUninstall,
  onSetup,
  menuRef,
}: {
  ext: ExtCardData;
  menuOpen: boolean;
  onToggle: () => void;
  onMenuToggle: () => void;
  onInstall: () => void;
  onUninstall: () => void;
  onSetup: () => void;
  menuRef?: React.RefObject<HTMLDivElement | null>;
}) {
  return (
    <div className="relative flex flex-1 flex-col gap-3 rounded-xl border border-border p-4">
      {/* 顶部：图标 + 信息 + 开关 */}
      <div className="flex items-start justify-between">
        <div className="flex items-center gap-3">
          <div className="flex size-10 items-center justify-center rounded-[10px] bg-[#EDE8FE]">
            <Puzzle className="size-[22px] text-[#8B6FC0]" />
          </div>
          <div className="flex flex-col gap-1">
            <span className="text-sm font-semibold text-foreground">
              {ext.name}
            </span>
            <span className="line-clamp-2 max-w-[160px] text-xs text-muted-foreground">
              {ext.description}
            </span>
          </div>
        </div>
        {ext.installed ? (
          <Switch checked={ext.enabled} onCheckedChange={onToggle} />
        ) : (
          <button
            onClick={onInstall}
            className="rounded-lg bg-primary px-3 py-1 text-xs font-medium text-primary-foreground transition-opacity hover:opacity-90"
          >
            安装
          </button>
        )}
      </div>

      {/* 底部：标签 + 更多 */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-1.5">
          <span className="rounded-md bg-secondary px-2.5 py-1 text-[11px] font-medium text-muted-foreground">
            v{ext.version}
          </span>
          {ext.installed && ext.enabled && (
            <span className="flex items-center gap-1 rounded-md bg-primary/10 px-2 py-1 text-[11px] font-medium text-primary">
              <ShieldCheck className="size-[11px]" />
              已启用
            </span>
          )}
        </div>
        {ext.installed && (
          <button
            onClick={onMenuToggle}
            className="text-muted-foreground transition-colors hover:text-foreground"
            aria-label="更多操作"
          >
            <MoreHorizontal className="size-[18px]" />
          </button>
        )}
      </div>

      {/* 弹出菜单 */}
      {menuOpen && (
        <div
          ref={menuRef}
          className="absolute right-4 top-full z-10 mt-1 w-[140px] rounded-[10px] border border-border bg-background p-1.5 shadow-lg"
        >
          <button
            onClick={onSetup}
            className="flex w-full items-center gap-2 rounded-md px-2.5 py-2 text-[13px] text-foreground transition-colors hover:bg-accent"
          >
            <Settings className="size-3.5 text-muted-foreground" />
            配置
          </button>
          <button className="flex w-full items-center gap-2 rounded-md px-2.5 py-2 text-[13px] text-foreground transition-colors hover:bg-accent">
            <Info className="size-3.5 text-muted-foreground" />
            查看详情
          </button>
          <div className="my-1 h-px bg-border" />
          <button
            onClick={onUninstall}
            className="flex w-full items-center gap-2 rounded-md bg-destructive/5 px-2.5 py-2 text-[13px] text-muted-foreground transition-colors hover:bg-destructive/10"
          >
            <Trash2 className="size-3.5" />
            卸载扩展
          </button>
        </div>
      )}
    </div>
  );
}
