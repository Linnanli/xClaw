/**
 * useExperimentalRuntime — dev-only 特性开关：是否启用 Phase 1 AI-SDK 新 Runtime 入口。
 *
 * 启用方式（三者任一）：
 *   1) URL query：`?runtime=experimental`（最高优先级，方便开发调试和 E2E）
 *   2) localStorage：`experimental-runtime=1`（调试会话内持久，开发者自行设置）
 *   3) Vite 构建开关：`VITE_EXPERIMENTAL_RUNTIME=1`（整个构建强制开启，CI / 内部 dogfood）
 *
 * 生产环境默认关闭。本 hook 只读，不写任何持久化存储——开发者需在浏览器控制台
 * 手动 `localStorage.setItem('experimental-runtime', '1')` 启用。
 *
 * 此开关属于 Phase 1 临时设施，Phase 1.5 切换 ChatTabTauri → ChatRuntimeProvider
 * 并删除旧 Provider 时，本 hook 和所有调用点一并移除。
 */

export function useExperimentalRuntime(): boolean {
  if (typeof window === 'undefined') return false;

  try {
    const params = new URLSearchParams(window.location.search);
    const q = params.get('runtime');
    if (q === 'experimental') return true;
    if (q === 'legacy') return false;

    const ls = window.localStorage?.getItem('experimental-runtime');
    if (ls === '1' || ls === 'true') return true;
  } catch {
    // SSR / storage 不可用时静默忽略
  }

  // Vite 构建开关
  try {
    const env = (import.meta as { env?: Record<string, string | undefined> }).env;
    if (env?.VITE_EXPERIMENTAL_RUNTIME === '1' || env?.VITE_EXPERIMENTAL_RUNTIME === 'true') {
      return true;
    }
  } catch {
    // ignore
  }

  return false;
}
