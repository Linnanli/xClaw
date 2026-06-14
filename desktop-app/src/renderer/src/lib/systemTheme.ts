type ThemeRoot = Pick<HTMLElement, 'classList'>
type MatchMedia = (query: string) => MediaQueryList

export function applySystemTheme(root: ThemeRoot, shouldUseDarkColors: boolean): void {
  root.classList.toggle('dark', shouldUseDarkColors)
}

export function watchSystemTheme({
  root = document.documentElement,
  matchMedia = window.matchMedia.bind(window)
}: {
  root?: ThemeRoot
  matchMedia?: MatchMedia
} = {}): () => void {
  const mediaQueryList = matchMedia('(prefers-color-scheme: dark)')
  const syncTheme = (): void => applySystemTheme(root, mediaQueryList.matches)

  syncTheme()
  mediaQueryList.addEventListener('change', syncTheme)

  return () => mediaQueryList.removeEventListener('change', syncTheme)
}
