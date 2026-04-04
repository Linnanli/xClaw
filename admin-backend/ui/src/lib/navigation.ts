/**
 * 在 React 组件树外部（如 axios 拦截器）使用 React Router 导航的桥接模块。
 *
 * 使用方式：
 * 1. 在 App.tsx 里调用 setNavigate(router.navigate)
 * 2. 在 api.ts 等非组件模块里调用 navigate('/login')
 */

type NavigateFn = (to: string, options?: { replace?: boolean }) => void

let _navigate: NavigateFn = (to) => {
  // 初始化前的 fallback
  window.location.replace(to)
}

export function setNavigate(fn: NavigateFn): void {
  _navigate = fn
}

export function navigate(to: string, options?: { replace?: boolean }): void {
  _navigate(to, options)
}
