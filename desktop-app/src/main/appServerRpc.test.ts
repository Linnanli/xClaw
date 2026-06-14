import { describe, expect, it } from 'vitest'

import { JsonRpcLineParser, buildJsonRpcRequestLine, classifyJsonRpcMessage } from './appServerRpc'

describe('app-server JSON-RPC helpers', () => {
  it('builds line-delimited JSON-RPC requests', () => {
    const line = buildJsonRpcRequestLine(7, 'turn/start', {
      threadId: 'thread-1',
      input: [{ type: 'text', text: 'hello' }]
    })

    expect(JSON.parse(line)).toEqual({
      jsonrpc: '2.0',
      id: 7,
      method: 'turn/start',
      params: {
        threadId: 'thread-1',
        input: [{ type: 'text', text: 'hello' }]
      }
    })
    expect(line.endsWith('\n')).toBe(true)
  })

  it('parses response and notification lines from arbitrary chunks', () => {
    const parser = new JsonRpcLineParser()

    expect(parser.push('{"jsonrpc":"2.0","id":1,"result":{"ok":')).toEqual([])

    const messages = parser.push(
      'true}}\n{"jsonrpc":"2.0","method":"turn/delta","params":{"delta":"Hi"}}\n'
    )

    expect(messages.map(classifyJsonRpcMessage)).toEqual([
      {
        type: 'response',
        id: 1,
        result: { ok: true }
      },
      {
        type: 'notification',
        method: 'turn/delta',
        params: { delta: 'Hi' }
      }
    ])
  })
})
