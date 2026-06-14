import { describe, expect, it } from 'vitest'

import { createMainWindowOptions } from './windowOptions'

describe('main window options', () => {
  it('opens the desktop app in fullscreen by default', () => {
    const options = createMainWindowOptions({
      preloadPath: '/app/preload/index.js',
      platform: 'darwin'
    })

    expect(options.fullscreen).toBe(true)
    expect(options.show).toBe(false)
  })

  it('keeps the Linux icon override without applying it to other platforms', () => {
    expect(
      createMainWindowOptions({
        preloadPath: '/app/preload/index.js',
        icon: '/app/icon.png',
        platform: 'linux'
      }).icon
    ).toBe('/app/icon.png')

    expect(
      createMainWindowOptions({
        preloadPath: '/app/preload/index.js',
        icon: '/app/icon.png',
        platform: 'darwin'
      }).icon
    ).toBeUndefined()
  })
})
