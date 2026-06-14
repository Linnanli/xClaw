// @vitest-environment jsdom

import { describe, expect, it, vi } from 'vitest'

import { watchSystemTheme } from './systemTheme'

type TestMediaQueryList = MediaQueryList & {
  emitChange: (matches: boolean) => void
}

function createMediaQueryList(initialMatches: boolean): TestMediaQueryList {
  let matches = initialMatches
  const listeners = new Set<(event: MediaQueryListEvent) => void>()

  return {
    media: '(prefers-color-scheme: dark)',
    get matches() {
      return matches
    },
    onchange: null,
    addEventListener: (_type, listener) => {
      listeners.add(listener as (event: MediaQueryListEvent) => void)
    },
    removeEventListener: (_type, listener) => {
      listeners.delete(listener as (event: MediaQueryListEvent) => void)
    },
    addListener: vi.fn(),
    removeListener: vi.fn(),
    dispatchEvent: () => true,
    emitChange: (nextMatches) => {
      const changeEvent = createMediaQueryChangeEvent(nextMatches)
      matches = nextMatches
      listeners.forEach((listener) => listener(changeEvent))
    }
  }
}

describe('system theme watcher', () => {
  it('applies the dark class when the system theme is dark', () => {
    const root = document.createElement('html')

    watchSystemTheme({
      root,
      matchMedia: () => createMediaQueryList(true)
    })

    expect(root.classList.contains('dark')).toBe(true)
  })

  it('updates the dark class when the system theme changes', () => {
    const root = document.createElement('html')
    const mediaQueryList = createMediaQueryList(false)

    const stopWatching = watchSystemTheme({
      root,
      matchMedia: () => mediaQueryList
    })

    expect(root.classList.contains('dark')).toBe(false)

    mediaQueryList.emitChange(true)
    expect(root.classList.contains('dark')).toBe(true)

    stopWatching()
    mediaQueryList.emitChange(false)
    expect(root.classList.contains('dark')).toBe(true)
  })
})

function createMediaQueryChangeEvent(matches: boolean): MediaQueryListEvent {
  const event = new Event('change') as MediaQueryListEvent
  Object.defineProperty(event, 'matches', { value: matches })
  Object.defineProperty(event, 'media', { value: '(prefers-color-scheme: dark)' })
  return event
}
