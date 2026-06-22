import { contextBridge, ipcRenderer } from 'electron'
import { electronAPI } from '@electron-toolkit/preload'
import type {
  AppServerNotification,
  AppServerRequestOptions,
  AppServerServerRequestResponse,
  AppServerStatus,
  DesktopAppServerApi
} from '../shared/appServerApi'

// Custom APIs for renderer
const desktopAppServer: DesktopAppServerApi = {
  request: <T = unknown>(method: string, params?: unknown, options: AppServerRequestOptions = {}) =>
    ipcRenderer.invoke('app-server:request', {
      method,
      params,
      hostId: options.hostId
    }) as Promise<T>,
  respondServerRequest: (
    requestId: string | number,
    response: AppServerServerRequestResponse,
    options: AppServerRequestOptions = {}
  ) =>
    ipcRenderer.invoke('app-server:respond-server-request', {
      requestId,
      response,
      hostId: options.hostId
    }) as Promise<void>,
  stop: () => ipcRenderer.invoke('app-server:stop'),
  getStatus: () => ipcRenderer.invoke('app-server:get-status'),
  checkHealth: () => ipcRenderer.invoke('app-server:check-health'),
  openExternalHttpUrl: (url: string) =>
    ipcRenderer.invoke('app-server:open-external-http-url', { url }) as Promise<void>,
  onStatusChange: (callback: (status: AppServerStatus) => void) => {
    const listener = (_event: Electron.IpcRendererEvent, status: AppServerStatus): void => {
      callback(status)
    }
    ipcRenderer.on('app-server:status-change', listener)
    return () => ipcRenderer.removeListener('app-server:status-change', listener)
  },
  onNotification: (callback: (notification: AppServerNotification) => void) => {
    const listener = (
      _event: Electron.IpcRendererEvent,
      notification: AppServerNotification
    ): void => {
      callback(notification)
    }
    ipcRenderer.on('app-server:notification', listener)
    return () => ipcRenderer.removeListener('app-server:notification', listener)
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
