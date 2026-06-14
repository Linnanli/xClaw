import { ElectronAPI } from '@electron-toolkit/preload'
import type { DesktopAppServerApi } from '../shared/appServerApi'

declare global {
  interface Window {
    electron: ElectronAPI
    desktopAppServer: DesktopAppServerApi
  }
}
