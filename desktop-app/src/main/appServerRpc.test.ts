import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { afterEach, describe, expect, it } from 'vitest'

import {
  JsonRpcLineParser,
  buildJsonRpcRequestLine,
  buildJsonRpcResponseLine,
  classifyJsonRpcMessage,
  resolveBundledAppServerBinary,
  resolveDefaultAppServerLaunchOptions
} from './appServerRpc'

const tempDirs: string[] = []

afterEach(() => {
  for (const dir of tempDirs.splice(0)) {
    rmSync(dir, { recursive: true, force: true })
  }
})

function tempResourcesDir(): string {
  const dir = mkdtempSync(join(tmpdir(), 'desktop-app-server-rpc-'))
  tempDirs.push(dir)
  return dir
}

function createBundledBinary(resourcesPath: string, platform: NodeJS.Platform): string {
  const bundleDir = join(resourcesPath, 'dasclaw-app-server')
  const binaryName = platform === 'win32' ? 'dasclaw-app-server.exe' : 'dasclaw-app-server'
  mkdirSync(bundleDir, { recursive: true })
  const binary = join(bundleDir, binaryName)
  writeFileSync(binary, 'binary')
  return binary
}

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
      'true}}\n{"jsonrpc":"2.0","method":"item/agentMessage/delta","params":{"delta":"Hi"}}\n'
    )

    expect(messages.map(classifyJsonRpcMessage)).toEqual([
      {
        type: 'response',
        id: 1,
        result: { ok: true }
      },
      {
        type: 'notification',
        method: 'item/agentMessage/delta',
        params: { delta: 'Hi' }
      }
    ])
  })

  it('classifies app-server JSON-RPC requests separately from notifications', () => {
    const message = classifyJsonRpcMessage({
      jsonrpc: '2.0',
      id: 'approval_1',
      method: 'item/commandExecution/requestApproval',
      params: {
        threadId: 'thread_1',
        turnId: 'turn_1',
        itemId: 'turn_1:tool:bash',
        toolCallId: 'call_1',
        toolName: 'bash',
        description: 'approve call to bash',
        displayParameters: { cmd: 'echo hello' },
        allowAlways: true
      }
    })

    expect(message).toEqual({
      type: 'server-request',
      id: 'approval_1',
      method: 'item/commandExecution/requestApproval',
      params: {
        threadId: 'thread_1',
        turnId: 'turn_1',
        itemId: 'turn_1:tool:bash',
        toolCallId: 'call_1',
        toolName: 'bash',
        description: 'approve call to bash',
        displayParameters: { cmd: 'echo hello' },
        allowAlways: true
      }
    })
  })

  it('builds app-server client response lines for approval decisions', () => {
    expect(
      buildJsonRpcResponseLine('approval_1', {
        decision: { kind: 'reject', data: { reason: 'not allowed' } }
      })
    ).toBe(
      '{"jsonrpc":"2.0","id":"approval_1","result":{"decision":{"kind":"reject","data":{"reason":"not allowed"}}}}\n'
    )
  })
})

describe('app-server process resolution', () => {
  it('uses the bundled app-server binary for packaged apps', () => {
    const resourcesPath = tempResourcesDir()
    const binary = createBundledBinary(resourcesPath, 'darwin')

    expect(resolveBundledAppServerBinary(resourcesPath, 'darwin')).toBe(binary)
    expect(
      resolveDefaultAppServerLaunchOptions({
        env: {},
        isPackaged: true,
        platform: 'darwin',
        resourcesPath
      })
    ).toEqual({
      command: binary,
      args: [],
      displayBinary: binary,
      env: {}
    })
  })

  it('uses cargo from the workspace root in development instead of a bare PATH lookup', () => {
    const mainDir = resolve('/repo/desktop-app/out/main')

    expect(
      resolveDefaultAppServerLaunchOptions({
        env: {},
        isPackaged: false,
        mainDir
      })
    ).toEqual({
      command: 'cargo',
      args: ['run', '--quiet', '-p', 'dasclaw_app_server', '--bin', 'dasclaw-app-server'],
      cwd: resolve('/repo'),
      displayBinary: 'cargo run --quiet -p dasclaw_app_server --bin dasclaw-app-server',
      env: {}
    })
  })

  it('lets DASCLAW_APP_SERVER_BIN override packaged and development resolution', () => {
    expect(
      resolveDefaultAppServerLaunchOptions({
        env: { DASCLAW_APP_SERVER_BIN: '/custom/dasclaw-app-server' },
        isPackaged: true,
        resourcesPath: '/missing'
      })
    ).toEqual({
      command: '/custom/dasclaw-app-server',
      args: [],
      displayBinary: '/custom/dasclaw-app-server',
      env: { DASCLAW_APP_SERVER_BIN: '/custom/dasclaw-app-server' }
    })
  })
})
