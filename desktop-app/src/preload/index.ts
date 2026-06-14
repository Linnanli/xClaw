import { contextBridge, ipcRenderer } from 'electron'
import { electronAPI } from '@electron-toolkit/preload'
import type { AppServerStatus, DesktopAppServerApi } from '../shared/appServerApi'

// Custom APIs for renderer
const desktopAppServer: DesktopAppServerApi = {
  start: () => ipcRenderer.invoke('app-server:start'),
  stop: () => ipcRenderer.invoke('app-server:stop'),
  getStatus: () => ipcRenderer.invoke('app-server:get-status'),
  checkHealth: () => ipcRenderer.invoke('app-server:check-health'),
  sendMessage: (prompt: string) => ipcRenderer.invoke('app-server:send-message', prompt),
  onStatusChange: (callback: (status: AppServerStatus) => void) => {
    const listener = (_event: Electron.IpcRendererEvent, status: AppServerStatus): void => {
      callback(status)
    }
    ipcRenderer.on('app-server:status-change', listener)
    return () => ipcRenderer.removeListener('app-server:status-change', listener)
  }
}

// Use `contextBridge` APIs to expose Electron APIs to
// renderer only if context isolation is enabled, otherwise
// just add to the DOM global.
if (process.contextIsolated) {
  try {
    contextBridge.exposeInMainWorld('electron', electronAPI)
    contextBridge.exposeInMainWorld('desktopAppServer', desktopAppServer)
  } catch (error) {
    console.error(error)
  }
} else {
  // @ts-ignore (define in dts)
  window.electron = electronAPI
  // @ts-ignore (define in dts)
  window.desktopAppServer = desktopAppServer
}
