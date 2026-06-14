import type { BrowserWindowConstructorOptions } from 'electron'

type MainWindowOptionsArgs = {
  preloadPath: string
  icon?: string
  platform?: NodeJS.Platform
}

export function createMainWindowOptions({
  preloadPath,
  icon,
  platform = process.platform
}: MainWindowOptionsArgs): BrowserWindowConstructorOptions {
  return {
    width: 900,
    height: 670,
    fullscreen: true,
    show: false,
    autoHideMenuBar: true,
    ...(platform === 'linux' ? { icon } : {}),
    webPreferences: {
      preload: preloadPath,
      sandbox: false
    }
  }
}
