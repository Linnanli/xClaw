import { afterEach, describe, expect, it, vi } from 'vitest'

import { runClientToolRequest } from './clientTools'
import type { AppServerServerRequest } from '../../../shared/appServerApi'

function toolRequest(tool: string, args: unknown): AppServerServerRequest<'item/tool/call'> {
  return {
    hostId: 'host_1',
    requestId: 'tool_1',
    method: 'item/tool/call',
    params: {
      threadId: 'thread_1',
      turnId: 'turn_1',
      callId: 'call_1',
      namespace: 'client',
      tool,
      arguments: args
    }
  }
}

describe('runClientToolRequest', () => {
  afterEach(() => {
    vi.unstubAllGlobals()
  })

  it('opens http URLs for open_url', async () => {
    const open = vi.fn()

    const response = await runClientToolRequest(
      toolRequest('open_url', {
        url: 'https://example.test/page?token=secret#fragment'
      }),
      { openExternal: open }
    )

    expect(open).toHaveBeenCalledWith('https://example.test/page?token=secret#fragment')
    expect(response).toEqual({
      contentItems: [{ type: 'inputText', text: 'Opened URL' }],
      success: true
    })
  })

  it('opens http URLs for client.open_url', async () => {
    const open = vi.fn()

    const response = await runClientToolRequest(
      toolRequest('client.open_url', {
        url: 'http://example.test/page'
      }),
      { openExternal: open }
    )

    expect(open).toHaveBeenCalledWith('http://example.test/page')
    expect(response).toEqual({
      contentItems: [{ type: 'inputText', text: 'Opened URL' }],
      success: true
    })
  })

  it('uses the typed desktopAppServer external opener by default', async () => {
    const openExternalHttpUrl = vi.fn().mockResolvedValue(undefined)
    vi.stubGlobal('window', {
      desktopAppServer: {
        openExternalHttpUrl
      }
    })

    const response = await runClientToolRequest(
      toolRequest('open_url', {
        url: 'https://example.test/default?token=secret#fragment'
      })
    )

    expect(openExternalHttpUrl).toHaveBeenCalledWith(
      'https://example.test/default?token=secret#fragment'
    )
    expect(response).toEqual({
      contentItems: [{ type: 'inputText', text: 'Opened URL' }],
      success: true
    })
  })

  it('rejects missing URL arguments', async () => {
    const open = vi.fn()

    const response = await runClientToolRequest(toolRequest('open_url', {}), {
      openExternal: open
    })

    expect(open).not.toHaveBeenCalled()
    expect(response).toEqual({
      contentItems: [{ type: 'inputText', text: 'open_url requires a string url argument' }],
      success: false
    })
  })

  it('rejects non-string URL arguments', async () => {
    const open = vi.fn()

    const response = await runClientToolRequest(toolRequest('open_url', { url: 42 }), {
      openExternal: open
    })

    expect(open).not.toHaveBeenCalled()
    expect(response).toEqual({
      contentItems: [{ type: 'inputText', text: 'open_url requires a string url argument' }],
      success: false
    })
  })

  it('returns a tool error when opening the URL fails', async () => {
    const open = vi.fn().mockRejectedValue(new Error('native shell failed'))

    const response = await runClientToolRequest(
      toolRequest('open_url', {
        url: 'https://example.test/page'
      }),
      { openExternal: open }
    )

    expect(open).toHaveBeenCalledWith('https://example.test/page')
    expect(response).toEqual({
      contentItems: [{ type: 'inputText', text: 'Failed to open URL' }],
      success: false
    })
  })

  it('returns a tool error when opening the URL throws synchronously', async () => {
    const open = vi.fn(() => {
      throw new Error('native shell failed')
    })

    const response = await runClientToolRequest(
      toolRequest('open_url', {
        url: 'https://example.test/page'
      }),
      { openExternal: open }
    )

    expect(open).toHaveBeenCalledWith('https://example.test/page')
    expect(response).toEqual({
      contentItems: [{ type: 'inputText', text: 'Failed to open URL' }],
      success: false
    })
  })

  it('rejects unsafe URL schemes', async () => {
    const open = vi.fn()

    const response = await runClientToolRequest(
      toolRequest('open_url', {
        url: 'javascript:alert(1)'
      }),
      { openExternal: open }
    )

    expect(open).not.toHaveBeenCalled()
    expect(response).toEqual({
      contentItems: [{ type: 'inputText', text: 'Rejected unsafe URL' }],
      success: false
    })
  })

  it('returns a tool error for unsupported client tools', async () => {
    const response = await runClientToolRequest(toolRequest('client.take_screenshot', {}), {
      openExternal: vi.fn()
    })

    expect(response).toEqual({
      contentItems: [
        { type: 'inputText', text: 'Unsupported client tool: client.take_screenshot' }
      ],
      success: false
    })
  })
})
