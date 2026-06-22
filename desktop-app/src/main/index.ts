import { app, shell, BrowserWindow, ipcMain, Menu, nativeTheme } from 'electron'
import { join } from 'path'
import { electronApp, optimizer, is } from '@electron-toolkit/utils'
import icon from '../../resources/icon.png?asset'
import { AppServerManager } from './appServerManager'
import { installWindowContextMenu } from './contextMenu'
import { createMainWindowOptions } from './windowOptions'
import type { AppServerServerRequestResponse } from '../shared/appServerApi'

const appServerManager = new AppServerManager()

function isExternalHttpUrl(value: string): boolean {
  try {
    const url = new URL(value)
    return url.protocol === 'http:' || url.protocol === 'https:'
  } catch {
    return false
  }
}

async function openExternalHttpUrl(url: string): Promise<void> {
  if (!isExternalHttpUrl(url)) {
    throw new Error('external URL must be http(s)')
  }

  try {
    await shell.openExternal(url)
  } catch {
    throw new Error('failed to open external URL')
  }
}

function createWindow(): void {
  // Create the browser window.
  const mainWindow = new BrowserWindow(
    createMainWindowOptions({
      preloadPath: join(__dirname, '../preload/index.js'),
      icon
    })
  )

  mainWindow.on('ready-to-show', () => {
    mainWindow.show()
  })

  mainWindow.webContents.setWindowOpenHandler((details) => {
    if (isExternalHttpUrl(details.url)) {
      void shell.openExternal(details.url).catch(() => {
        console.error('failed to open external URL')
      })
    }
    return { action: 'deny' }
  })
  installWindowContextMenu(mainWindow, Menu)

  const unsubscribeStatus = appServerManager.onStatusChange((status) => {
    if (!mainWindow.isDestroyed()) {
      mainWindow.webContents.send('app-server:status-change', status)
    }
  })
  const unsubscribeNotifications = appServerManager.onNotification((notification) => {
    if (!mainWindow.isDestroyed()) {
      mainWindow.webContents.send('app-server:notification', notification)
    }
  })
  mainWindow.on('closed', () => {
    unsubscribeStatus()
    unsubscribeNotifications()
  })

  // HMR for renderer base on electron-vite cli.
  // Load the remote URL for development or the local html file for production.
  if (is.dev && process.env['ELECTRON_RENDERER_URL']) {
    mainWindow.loadURL(process.env['ELECTRON_RENDERER_URL'])
  } else {
    mainWindow.loadFile(join(__dirname, '../renderer/index.html'))
  }
}

// This method will be called when Electron has finished
// initialization and is ready to create browser windows.
// Some APIs can only be used after this event occurs.
app.whenReady().then(() => {
  // Set app user model id for windows
  electronApp.setAppUserModelId('com.electron')
  nativeTheme.themeSource = 'system'

  // Default open or close DevTools by F12 in development
  // and ignore CommandOrControl + R in production.
  // see https://github.com/alex8088/electron-toolkit/tree/master/packages/utils
  app.on('browser-window-created', (_, window) => {
    optimizer.watchWindowShortcuts(window)
  })

  // IPC test
  ipcMain.on('ping', () => console.log('pong'))
  ipcMain.handle('app-server:request', (_, payload: unknown) => {
    if (!payload || typeof payload !== 'object') {
      throw new Error('app-server request payload must be an object')
    }
    const request = payload as { method?: unknown; params?: unknown; hostId?: unknown }
    if (typeof request.method !== 'string') {
      throw new Error('app-server request method must be a string')
    }
    if (request.hostId !== undefined && typeof request.hostId !== 'string') {
      throw new Error('app-server request hostId must be a string')
    }
    return appServerManager.request(request.method, request.params, { hostId: request.hostId })
  })
  ipcMain.handle('app-server:respond-server-request', (_, payload: unknown) => {
    if (!payload || typeof payload !== 'object') {
      throw new Error('app-server response payload must be an object')
    }
    const responsePayload = payload as {
      requestId?: unknown
      response?: AppServerServerRequestResponse
      hostId?: unknown
    }
    if (
      typeof responsePayload.requestId !== 'string' &&
      typeof responsePayload.requestId !== 'number'
    ) {
      throw new Error('app-server response requestId must be a string or number')
    }
    if (responsePayload.hostId !== undefined && typeof responsePayload.hostId !== 'string') {
      throw new Error('app-server response hostId must be a string')
    }
    return appServerManager.respondServerRequest(
      responsePayload.requestId,
      responsePayload.response as AppServerServerRequestResponse,
      { hostId: responsePayload.hostId }
    )
  })
  ipcMain.handle('app-server:stop', () => appServerManager.stop())
  ipcMain.handle('app-server:get-status', () => appServerManager.getStatus())
  ipcMain.handle('app-server:check-health', () => appServerManager.checkHealth())
  ipcMain.handle('app-server:open-external-http-url', (_, payload: unknown) => {
    if (!payload || typeof payload !== 'object') {
      throw new Error('external URL payload must be an object')
    }
    const request = payload as { url?: unknown }
    if (typeof request.url !== 'string') {
      throw new Error('external URL must be a string')
    }
    return openExternalHttpUrl(request.url)
  })

  createWindow()
  void appServerManager.preconnect()

  app.on('activate', function () {
    // On macOS it's common to re-create a window in the app when the
    // dock icon is clicked and there are no other windows open.
    if (BrowserWindow.getAllWindows().length === 0) createWindow()
  })
})

// Quit when all windows are closed, except on macOS. There, it's common
// for applications and their menu bar to stay active until the user quits
// explicitly with Cmd + Q.
app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') {
    app.quit()
  }
})

app.on('before-quit', () => {
  void appServerManager.stop()
})

// In this file you can include the rest of your app's specific main process
// code. You can also put them in separate files and require them here.
