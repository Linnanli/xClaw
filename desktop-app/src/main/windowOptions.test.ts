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

  it('enables the macOS vibrancy backdrop for translucent UI surfaces', () => {
    const options = createMainWindowOptions({
      preloadPath: '/app/preload/index.js',
      platform: 'darwin'
    })

    expect(options.backgroundColor).toBe('#00000000')
    expect(options.transparent).toBe(true)
    expect(options.vibrancy).toBe('menu')
    expect(options.visualEffectState).toBe('active')
    expect(options.titleBarStyle).toBe('hiddenInset')
  })

  it('keeps Windows on the default opaque window colors', () => {
    const options = createMainWindowOptions({
      preloadPath: '/app/preload/index.js',
      platform: 'win32'
    })

    expect(options.backgroundColor).toBeUndefined()
    expect(options.backgroundMaterial).toBeUndefined()
    expect(options.titleBarStyle).toBeUndefined()
    expect(options.titleBarOverlay).toBeUndefined()
    expect(options.transparent).toBeUndefined()
  })

  it('keeps Linux on the default opaque window colors', () => {
    const options = createMainWindowOptions({
      preloadPath: '/app/preload/index.js',
      platform: 'linux'
    })

    expect(options.backgroundColor).toBeUndefined()
    expect(options.transparent).toBeUndefined()
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
