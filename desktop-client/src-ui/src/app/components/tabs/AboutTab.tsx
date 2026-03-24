/**
 * AboutTab - 关于我们面板
 *
 * 设计稿：垂直居中布局
 * - LogoSection: 88px 圆形绿色背景 + shield-check 图标 + 应用名 + 副标题
 * - VersionRow: 圆角灰色背景条，左侧版本号 + 右侧绿色"检查更新"按钮
 */

import { useState } from 'react';
import { ShieldCheck, RefreshCw } from 'lucide-react';

export function AboutTab() {
  const [checking, setChecking] = useState(false);
  const [checkResult, setCheckResult] = useState<string | null>(null);

  const handleCheckUpdate = async () => {
    setChecking(true);
    try {
      // 模拟检查更新
      await new Promise((r) => setTimeout(r, 1500));
      setCheckResult('当前已是最新版本');
    } catch {
      setCheckResult('检查更新失败');
    } finally {
      setChecking(false);
    }
  };

  return (
    <div className="flex h-full min-h-[480px] flex-col items-center justify-center gap-7 px-12">
      {/* Logo Section */}
      <div className="flex flex-col items-center gap-3">
        <div className="flex size-[88px] items-center justify-center rounded-full bg-[#E8F5EE]">
          <ShieldCheck className="size-11 text-primary" />
        </div>
        <h3
          className="text-[26px] font-bold tracking-tight text-foreground"
          style={{ letterSpacing: '-0.6px' }}
        >
          X-Claw
        </h3>
        <p className="text-[13px] text-[#9D9C9A]">
          智能 AI 助手，让工作更高效
        </p>
      </div>

      {/* Version Row */}
      <div className="flex h-14 w-full items-center justify-between rounded-xl bg-[#F5F4F1] px-5">
        <div className="flex items-center gap-2">
          <span className="text-sm font-semibold text-foreground">当前版本</span>
          <span className="text-sm text-[#6D6C6A]">v0.1.0</span>
        </div>
        <button
          onClick={handleCheckUpdate}
          disabled={checking}
          className="flex items-center gap-1.5 rounded-lg bg-primary px-4 py-2 text-[13px] font-semibold text-white transition-colors hover:bg-primary/90 disabled:opacity-60"
        >
          <RefreshCw className={`size-3.5 ${checking ? 'animate-spin' : ''}`} />
          {checking ? '检查中...' : '检查更新'}
        </button>
      </div>

      {/* Check Result - 始终占位，避免布局抖动 */}
      <p className={`text-xs text-[#9D9C9A] transition-opacity duration-200 ${checkResult ? 'opacity-100' : 'opacity-0'}`}>
        {checkResult ?? '\u00A0'}
      </p>
    </div>
  );
}
