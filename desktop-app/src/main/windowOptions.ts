import type { BrowserWindowConstructorOptions } from 'electron'

type MainWindowOptionsArgs = {
  preloadPath: string
  icon?: string
  platform?: NodeJS.Platform
}

function createNativeBackdropWindowOptions(
  platform: NodeJS.Platform
): Pick<
  BrowserWindowConstructorOptions,
  | 'backgroundColor'
  | 'titleBarStyle'
  | 'trafficLightPosition'
  | 'transparent'
  | 'vibrancy'
  | 'visualEffectState'
> {
  if (platform === 'darwin') {
    return {
      backgroundColor: '#00000000',
      titleBarStyle: 'hiddenInset',
      trafficLightPosition: { x: 16, y: 16 },
      transparent: true,
      vibrancy: 'menu',
      visualEffectState: 'active'
    }
  }

  return {}
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
    ...createNativeBackdropWindowOptions(platform),
    ...(platform === 'linux' ? { icon } : {}),
    webPreferences: {
      preload: preloadPath,
      sandbox: false
    }
  }
}
