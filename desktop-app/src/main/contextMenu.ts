import type { BrowserWindow, MenuItemConstructorOptions } from 'electron'

type ContextMenuPoint = {
  x: number
  y: number
}

type ContextMenuWebContents = {
  inspectElement(x: number, y: number): void
  openDevTools(options: { mode: 'undocked' }): void
  reload(): void
  on(channel: 'context-menu', listener: (event: unknown, params: ContextMenuPoint) => void): void
}

type PopupMenu = {
  popup(options: { window?: BrowserWindow }): void
}

type MenuBuilder = {
  buildFromTemplate(template: MenuItemConstructorOptions[]): PopupMenu
}

export function createWindowContextMenuTemplate(
  webContents: Pick<ContextMenuWebContents, 'inspectElement' | 'openDevTools' | 'reload'>,
  point: ContextMenuPoint
): MenuItemConstructorOptions[] {
  return [
    { role: 'copy' },
    { role: 'paste' },
    { type: 'separator' },
    {
      label: '检查元素',
      click: () => webContents.inspectElement(point.x, point.y)
    },
    {
      label: '打开控制台',
      click: () => webContents.openDevTools({ mode: 'undocked' })
    },
    {
      label: '刷新页面',
      click: () => webContents.reload()
    }
  ]
}

export function installWindowContextMenu(
  mainWindow: BrowserWindow,
  menuBuilder: MenuBuilder
): void {
  const webContents = mainWindow.webContents as ContextMenuWebContents

  webContents.on('context-menu', (_event, params) => {
    const menu = menuBuilder.buildFromTemplate(createWindowContextMenuTemplate(webContents, params))
    menu.popup({ window: mainWindow })
  })
}
