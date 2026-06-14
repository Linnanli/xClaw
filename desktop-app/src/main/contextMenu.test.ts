import { describe, expect, it, vi } from 'vitest'

import { createWindowContextMenuTemplate } from './contextMenu'

function clickMenuItem(
  template: ReturnType<typeof createWindowContextMenuTemplate>,
  label: string
): void {
  const item = template.find((entry) => entry.label === label)

  expect(item?.click).toBeTypeOf('function')
  ;(item?.click as () => void)()
}

describe('window context menu', () => {
  it('offers inspect element and reload actions for the clicked position', () => {
    const inspectElement = vi.fn()
    const openDevTools = vi.fn()
    const reload = vi.fn()

    const template = createWindowContextMenuTemplate(
      { inspectElement, openDevTools, reload },
      { x: 12, y: 34 }
    )

    expect(template.map((item) => item.label ?? item.role ?? item.type)).toEqual([
      'copy',
      'paste',
      'separator',
      '检查元素',
      '打开控制台',
      '刷新页面'
    ])

    clickMenuItem(template, '检查元素')
    clickMenuItem(template, '打开控制台')
    clickMenuItem(template, '刷新页面')

    expect(inspectElement).toHaveBeenCalledWith(12, 34)
    expect(openDevTools).toHaveBeenCalledWith({ mode: 'undocked' })
    expect(reload).toHaveBeenCalledOnce()
  })
})
