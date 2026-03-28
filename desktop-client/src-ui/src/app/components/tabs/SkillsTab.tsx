/**
 * SkillsTab - 技能管理面板
 *
 * 展示本地已加载的技能（workspace 内置 + 用户安装）。
 * 数据来源：ic_list_skills（直接读取 SkillRegistry，包含 workspace_dir 中的内置技能）。
 *
 * 设计：搜索栏 + 2 列卡片网格
 * 每张卡片：图标 + 名称 + 描述 + 来源标签 + 信任标签 + 关键词 + 更多菜单
 */

import { useState, useEffect, useCallback, useRef } from 'react';
import {
  Search,
  Puzzle,
  ShieldCheck,
  MoreHorizontal,
  Info,
  Trash2,
  Lock,
  Zap,
} from 'lucide-react';
import { cn } from '../ui/utils';
import { invokeTauri } from '../../utils/tauri';

// ── 类型定义（与 Rust SkillInfo 契约对齐）──────────────────────────

interface SkillInfo {
  name: string;
  version: string;
  description: string;
  /** "workspace" | "user" | "installed" */
  source: string;
  /** "trusted" | "installed" */
  trust: string;
  keywords: string[];
}

// ── 主组件 ────────────────────────────────────────────────────────

export function SkillsTab() {
  const [skills, setSkills] = useState<SkillInfo[]>([]);
  const [searchQuery, setSearchQuery] = useState('');
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [menuOpenId, setMenuOpenId] = useState<string | null>(null);
  const menuRef = useRef<HTMLDivElement>(null);

  const loadSkills = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const result = await invokeTauri<SkillInfo[]>('ic_list_skills');
      setSkills(result);
    } catch (err) {
      console.error('Failed to load skills:', err);
      setError('加载技能失败');
      setSkills([]);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadSkills();
  }, [loadSkills]);

  // 点击外部关闭菜单
  useEffect(() => {
    if (!menuOpenId) return;
    const handleClickOutside = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setMenuOpenId(null);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [menuOpenId]);

  const handleUninstall = async (name: string) => {
    setMenuOpenId(null);
    try {
      await invokeTauri('ic_uninstall_skill', { name });
      await loadSkills();
    } catch (err) {
      console.error('Failed to uninstall skill:', err);
      setError('卸载技能失败');
    }
  };

  const filtered = skills.filter(
    (s) =>
      !searchQuery.trim() ||
      s.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
      s.description.toLowerCase().includes(searchQuery.toLowerCase()) ||
      s.keywords.some((k) => k.toLowerCase().includes(searchQuery.toLowerCase())),
  );

  // 两列分组
  const rows: SkillInfo[][] = [];
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

      {/* 搜索栏 */}
      <div className="relative">
        <Search className="absolute left-3.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
        <input
          type="text"
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          placeholder="搜索技能名称、描述或关键词"
          className="h-10 w-full rounded-[10px] border border-border bg-secondary/50 pl-10 pr-3 text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-primary/30"
        />
      </div>

      {/* 技能卡片网格 */}
      {loading ? (
        <div className="flex items-center justify-center py-16 text-sm text-muted-foreground">
          加载中...
        </div>
      ) : filtered.length === 0 ? (
        <EmptyState hasQuery={!!searchQuery.trim()} />
      ) : (
        <div className="flex flex-col gap-3">
          {rows.map((row, ri) => (
            <div key={ri} className="flex gap-3">
              {row.map((skill) => (
                <SkillCard
                  key={skill.name}
                  skill={skill}
                  menuOpen={menuOpenId === skill.name}
                  onMenuToggle={() =>
                    setMenuOpenId(menuOpenId === skill.name ? null : skill.name)
                  }
                  onUninstall={() => handleUninstall(skill.name)}
                  menuRef={menuOpenId === skill.name ? menuRef : undefined}
                />
              ))}
              {row.length === 1 && <div className="flex-1" />}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

// ── 空状态 ────────────────────────────────────────────────────────

function EmptyState({ hasQuery }: { hasQuery: boolean }) {
  return (
    <div className="flex flex-col items-center justify-center gap-3 py-16 text-center">
      <Puzzle className="size-10 text-muted-foreground/40" />
      <p className="text-sm text-muted-foreground">
        {hasQuery ? '未找到匹配的技能' : '暂无已加载的技能'}
      </p>
      {!hasQuery && (
        <p className="max-w-[260px] text-xs text-muted-foreground/70">
          将 SKILL.md 文件放入 <code className="rounded bg-secondary px-1">~/.ironclaw/skills/</code> 目录，重启后即可加载
        </p>
      )}
    </div>
  );
}

// ── 技能卡片 ──────────────────────────────────────────────────────

function SkillCard({
  skill,
  menuOpen,
  onMenuToggle,
  onUninstall,
  menuRef,
}: {
  skill: SkillInfo;
  menuOpen: boolean;
  onMenuToggle: () => void;
  onUninstall: () => void;
  menuRef?: React.RefObject<HTMLDivElement | null>;
}) {
  const isBuiltin = skill.source === 'workspace';
  const isTrusted = skill.trust === 'trusted';

  return (
    <div className="relative flex flex-1 flex-col gap-3 rounded-xl border border-border p-4">
      {/* 顶部：图标 + 名称/描述 + 更多按钮 */}
      <div className="flex items-start justify-between gap-2">
        <div className="flex items-start gap-3">
          <div className="flex size-10 shrink-0 items-center justify-center rounded-[10px] bg-[#FEF0E8]">
            <Puzzle className="size-[22px] text-[#D89575]" />
          </div>
          <div className="flex flex-col gap-0.5">
            <span className="text-sm font-semibold text-foreground">{skill.name}</span>
            <span className="line-clamp-2 max-w-[180px] text-xs text-muted-foreground">
              {skill.description || '暂无描述'}
            </span>
          </div>
        </div>
        <button
          onClick={onMenuToggle}
          className="shrink-0 text-muted-foreground transition-colors hover:text-foreground"
          aria-label="更多操作"
        >
          <MoreHorizontal className="size-[18px]" />
        </button>
      </div>

      {/* 底部：标签 + 关键词 */}
      <div className="flex flex-wrap items-center gap-1.5">
        <SourceBadge source={skill.source} />
        {isTrusted && <TrustedBadge />}
        {skill.keywords.slice(0, 3).map((kw) => (
          <KeywordBadge key={kw} keyword={kw} />
        ))}
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
            onClick={isBuiltin ? undefined : onUninstall}
            disabled={isBuiltin}
            className={cn(
              'flex w-full items-center gap-2 rounded-md px-2.5 py-2 text-[13px] transition-colors',
              isBuiltin
                ? 'cursor-not-allowed text-muted-foreground/40'
                : 'text-muted-foreground hover:bg-destructive/10 hover:text-destructive',
            )}
          >
            <Trash2 className="size-3.5" />
            移除技能
            {isBuiltin && <Lock className="ml-auto size-[11px]" />}
          </button>
        </div>
      )}
    </div>
  );
}

// ── 小型标签组件 ──────────────────────────────────────────────────

function SourceBadge({ source }: { source: string }) {
  const label =
    source === 'workspace' ? '内置技能' : source === 'user' ? '用户技能' : '已安装';
  return (
    <span className="rounded-md bg-secondary px-2 py-0.5 text-[11px] font-medium text-muted-foreground">
      {label}
    </span>
  );
}

function TrustedBadge() {
  return (
    <span className="flex items-center gap-1 rounded-md bg-primary/10 px-2 py-0.5 text-[11px] font-medium text-primary">
      <ShieldCheck className="size-[11px]" />
      受信任
    </span>
  );
}

function KeywordBadge({ keyword }: { keyword: string }) {
  return (
    <span className="flex items-center gap-0.5 rounded-md bg-accent px-2 py-0.5 text-[11px] text-muted-foreground">
      <Zap className="size-[10px]" />
      {keyword}
    </span>
  );
}
