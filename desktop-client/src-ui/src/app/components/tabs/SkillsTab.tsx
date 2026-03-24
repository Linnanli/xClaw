/**
 * SkillsTab - 技能管理面板
 *
 * 设计稿：搜索栏 + 添加技能按钮 + 2列卡片网格
 * 每张卡片：图标 + 名称 + 描述 + 开关 + 标签（内置技能/安全审核）+ 更多菜单
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
  Lock,
} from 'lucide-react';
import { Switch } from '../ui/switch';
import { cn } from '../ui/utils';
import { skillApi, type Skill, type InstalledSkill } from '../../utils/tauri';

/* ── 类型定义 ── */

interface SkillCardData {
  id: string;
  name: string;
  description: string;
  enabled: boolean;
  source: 'builtin' | 'community';
  trustLevel: 'high' | 'medium' | 'low';
}

/* ── 主组件 ── */

export function SkillsTab() {
  const [skills, setSkills] = useState<SkillCardData[]>([]);
  const [searchQuery, setSearchQuery] = useState('');
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [menuOpenId, setMenuOpenId] = useState<string | null>(null);
  const menuRef = useRef<HTMLDivElement>(null);

  const loadSkills = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const [installed, available] = await Promise.all([
        skillApi.getInstalledSkills(),
        skillApi.getAvailableSkills(),
      ]);

      const installedMap = new Map(
        installed.map((s: InstalledSkill) => [s.metadata.id, s]),
      );

      const merged: SkillCardData[] = available.map((s: Skill) => {
        const inst = installedMap.get(s.id);
        return {
          id: s.id,
          name: s.name,
          description: s.description,
          enabled: inst?.enabled ?? false,
          source: s.source === 'builtin' ? 'builtin' : 'community',
          trustLevel:
            s.trust_level === 'high'
              ? 'high'
              : s.trust_level === 'low'
                ? 'low'
                : 'medium',
        } satisfies SkillCardData;
      });

      // 补充已安装但不在 available 列表中的
      for (const [id, inst] of installedMap) {
        if (!merged.some((m) => m.id === id)) {
          merged.push({
            id,
            name: inst.metadata.name,
            description: inst.metadata.description,
            enabled: inst.enabled,
            source:
              inst.metadata.source === 'builtin' ? 'builtin' : 'community',
            trustLevel:
              inst.metadata.trust_level === 'high'
                ? 'high'
                : inst.metadata.trust_level === 'low'
                  ? 'low'
                  : 'medium',
          });
        }
      }

      setSkills(merged);
    } catch (err) {
      console.error('Failed to load skills:', err);
      setError('加载技能失败');
      // fallback 示例数据
      setSkills([
        {
          id: 'agent-mbti',
          name: 'agent-mbti',
          description:
            'AI Agent personality diagnosis and configuration system bas...',
          enabled: true,
          source: 'builtin',
          trustLevel: 'high',
        },
        {
          id: 'algorithmic-art',
          name: 'algorithmic-art',
          description:
            'Creating algorithmic art using p5.js with seeded randomnes...',
          enabled: true,
          source: 'builtin',
          trustLevel: 'high',
        },
        {
          id: 'analytics-dashboard',
          name: 'analytics-dashboard',
          description:
            '数据看板。业务数据看板生成器。自动搭建KPI监控看板、实时指...',
          enabled: true,
          source: 'builtin',
          trustLevel: 'high',
        },
        {
          id: 'arxiv-reader',
          name: 'arxiv-reader',
          description:
            'arXiv论文智能阅读助手。通过对话式交互阅读和分析学术论文。...',
          enabled: true,
          source: 'builtin',
          trustLevel: 'high',
        },
      ]);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadSkills();
  }, [loadSkills]);

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
    // 乐观更新
    setSkills((prev) =>
      prev.map((s) => (s.id === id ? { ...s, enabled: !currentEnabled } : s)),
    );
    try {
      if (currentEnabled) {
        await skillApi.disableSkill(id);
      } else {
        await skillApi.enableSkill(id);
      }
    } catch {
      // 回滚
      setSkills((prev) =>
        prev.map((s) =>
          s.id === id ? { ...s, enabled: currentEnabled } : s,
        ),
      );
    }
  };

  const handleUninstall = async (id: string) => {
    setMenuOpenId(null);
    try {
      await skillApi.uninstallSkill(id);
      await loadSkills();
    } catch (err) {
      console.error('Failed to uninstall skill:', err);
    }
  };

  const filtered = skills.filter(
    (s) =>
      !searchQuery.trim() ||
      s.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
      s.description.toLowerCase().includes(searchQuery.toLowerCase()),
  );

  // 两列分组
  const rows: SkillCardData[][] = [];
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
            placeholder="搜索已经安装的技能"
            className="h-10 w-full rounded-[10px] border border-border bg-secondary/50 pl-10 pr-3 text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-primary/30"
          />
        </div>
        <button className="flex h-10 items-center gap-1.5 rounded-[10px] bg-primary px-4 text-sm font-semibold text-primary-foreground transition-opacity hover:opacity-90">
          <Plus className="size-4" />
          添加技能
        </button>
      </div>

      {/* 技能卡片网格 */}
      {loading ? (
        <div className="flex items-center justify-center py-16 text-sm text-muted-foreground">
          加载中...
        </div>
      ) : filtered.length === 0 ? (
        <div className="flex items-center justify-center py-16 text-sm text-muted-foreground">
          {searchQuery ? '未找到匹配的技能' : '暂无技能'}
        </div>
      ) : (
        <div className="flex flex-col gap-3">
          {rows.map((row, ri) => (
            <div key={ri} className="flex gap-3">
              {row.map((skill) => (
                <SkillCard
                  key={skill.id}
                  skill={skill}
                  menuOpen={menuOpenId === skill.id}
                  onToggle={() => handleToggle(skill.id, skill.enabled)}
                  onMenuToggle={() =>
                    setMenuOpenId(menuOpenId === skill.id ? null : skill.id)
                  }
                  onUninstall={() => handleUninstall(skill.id)}
                  menuRef={menuOpenId === skill.id ? menuRef : undefined}
                />
              ))}
              {/* 占位保持两列对齐 */}
              {row.length === 1 && <div className="flex-1" />}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

/* ── 技能卡片 ── */

function SkillCard({
  skill,
  menuOpen,
  onToggle,
  onMenuToggle,
  onUninstall,
  menuRef,
}: {
  skill: SkillCardData;
  menuOpen: boolean;
  onToggle: () => void;
  onMenuToggle: () => void;
  onUninstall: () => void;
  menuRef?: React.RefObject<HTMLDivElement | null>;
}) {
  return (
    <div className="relative flex flex-1 flex-col gap-3 rounded-xl border border-border p-4">
      {/* 顶部：图标 + 信息 + 开关 */}
      <div className="flex items-start justify-between">
        <div className="flex items-center gap-3">
          <div className="flex size-10 items-center justify-center rounded-[10px] bg-[#FEF0E8]">
            <Puzzle className="size-[22px] text-[#D89575]" />
          </div>
          <div className="flex flex-col gap-1">
            <span className="text-sm font-semibold text-foreground">
              {skill.name}
            </span>
            <span className="line-clamp-2 max-w-[160px] text-xs text-muted-foreground">
              {skill.description}
            </span>
          </div>
        </div>
        <Switch checked={skill.enabled} onCheckedChange={onToggle} />
      </div>

      {/* 底部：标签 + 更多 */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-1.5">
          <span className="rounded-md bg-secondary px-2.5 py-1 text-[11px] font-medium text-muted-foreground">
            {skill.source === 'builtin' ? '内置技能' : '社区技能'}
          </span>
          {skill.trustLevel === 'high' && (
            <span className="flex items-center gap-1 rounded-md bg-primary/10 px-2 py-1 text-[11px] font-medium text-primary">
              <ShieldCheck className="size-[11px]" />
              安全审核
            </span>
          )}
        </div>
        <button
          onClick={onMenuToggle}
          className="text-muted-foreground transition-colors hover:text-foreground"
          aria-label="更多操作"
        >
          <MoreHorizontal className="size-[18px]" />
        </button>
      </div>

      {/* 弹出菜单 */}
      {menuOpen && (
        <div
          ref={menuRef}
          className="absolute right-4 top-full z-10 mt-1 w-[140px] rounded-[10px] border border-border bg-background p-1.5 shadow-lg"
        >
          <button className="flex w-full items-center gap-2 rounded-md px-2.5 py-2 text-[13px] text-foreground transition-colors hover:bg-accent">
            <Info className="size-3.5 text-muted-foreground" />
            查看详情
          </button>
          <div className="my-1 h-px bg-border" />
          <button
            onClick={onUninstall}
            className={cn(
              'flex w-full items-center gap-2 rounded-md bg-destructive/5 px-2.5 py-2 text-[13px] transition-colors',
              skill.source === 'builtin'
                ? 'cursor-not-allowed text-muted-foreground/50'
                : 'text-muted-foreground hover:bg-destructive/10',
            )}
            disabled={skill.source === 'builtin'}
          >
            <Trash2 className="size-3.5" />
            移除技能
            {skill.source === 'builtin' && <Lock className="size-[11px]" />}
          </button>
        </div>
      )}
    </div>
  );
}
