import type {
  AppServerDynamicToolCallResponse,
  AppServerServerRequest
} from '../../../shared/appServerApi'

type ClientToolRuntime = {
  openExternal: (url: string) => void | Promise<void>
}

export async function runClientToolRequest(
  request: AppServerServerRequest<'item/tool/call'>,
  runtime: ClientToolRuntime = {
    openExternal: (url) => window.desktopAppServer.openExternalHttpUrl(url)
  }
): Promise<AppServerDynamicToolCallResponse> {
  const tool = request.params.tool
  if (tool !== 'open_url' && tool !== 'client.open_url') {
    return toolError(`Unsupported client tool: ${tool}`)
  }

  const url = readUrl(request.params.arguments)
  if (!url) {
    return toolError('open_url requires a string url argument')
  }
  if (!isSafeExternalUrl(url)) {
    return toolError('Rejected unsafe URL')
  }

  try {
    await runtime.openExternal(url)
  } catch {
    return toolError('Failed to open URL')
  }

  return {
    contentItems: [{ type: 'inputText', text: 'Opened URL' }],
    success: true
  }
}

function readUrl(value: unknown): string | undefined {
  if (!value || typeof value !== 'object') return undefined
  const url = (value as { url?: unknown }).url
  return typeof url === 'string' ? url : undefined
}

function isSafeExternalUrl(value: string): boolean {
  try {
    const url = new URL(value)
    return url.protocol === 'http:' || url.protocol === 'https:'
  } catch {
    return false
  }
}

function toolError(text: string): AppServerDynamicToolCallResponse {
  return {
    contentItems: [{ type: 'inputText', text }],
    success: false
  }
}
