# Dasclaw App Server R1 Product Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete R1 as an end-to-end product capability by replacing renderer fail-closed placeholders with real user interaction, a minimal client tool executor, and real runtime response plumbing for client dynamic tools.

**Architecture:** Keep the existing app-server JSON-RPC ServerRequest owner intact. Add a small typed renderer request queue and UI surface in `desktop-app`, and refactor `DasclawAgentRuntimeBridge` so client dynamic tool calls suspend through a bridge-owned pending response map instead of treating the client response as a no-op. File-change/user-input/auto-review remain explicit product surfaces: renderer must show them, respond with user intent, and tests must stop treating automatic reject/empty answers as product completion.

**Tech Stack:** Rust 2024, `dasclaw_app_server`, `dasclaw_app_server_protocol`, `dasclaw_runtime`, Electron main process TypeScript, React 19, assistant-ui, lucide-react, Vitest, Cargo nextest.

---

## Scope And Evidence

### Task Start 4 Questions

1. **是否新增模块 / crate / 文件？** 是。新增 renderer helper/component/test 文件；已用 `semantic_search_nodes_tool` 搜索 R1 runtime/tool/user-input/file-change/auto-review 相关实现，结论是协议和测试桥已有，真实 renderer/product path 未完整落地。
2. **结论里是否含否定语？** 是。计划会指出真实 renderer UI 和 client tool executor 尚未完成；证据来自语义搜索、LSP `execute_lsp workspace_symbols`、`rg` 精确定位。
3. **是否做跨项目对账？** 否。本计划只面向当前 `x-claw` worktree 的 `dasclaw_app_server` 和 `desktop-app`。
4. **是否写架构对账类文档？** 是。本文件是实施计划；证据层如下。

### Evidence Used To Define Remaining R1

- Semantic layer: `semantic_search_nodes_tool` query `R1 runtime tool user input file change approval auto approval review producer renderer UI client tool executor` returned existing approval/tool primitives and test-oriented R1 surfaces, but not a completed renderer product flow for the new app-server path.
- Symbol layer: local `lsp-mcp execute_lsp workspace_symbols` found `RuntimeBridge::resolve_server_request`, `DasclawAgentRuntimeBridge::resolve_server_request`, and test `R1RuntimeBridge` in `/Users/nallylin/Documents/code/x-claw/crates/dasclaw_app_server/src/lib.rs`; it also found renderer `failClosedRendererServerRequestResponse()` in `/Users/nallylin/Documents/code/x-claw/desktop-app/src/renderer/src/hooks/useDasclawAssistantRuntime.ts`.
- Literal layer: `rg` shows `DasclawAgentRuntimeBridge::resolve_server_request` returns `Ok(())` for `DynamicTool` and fatal unsupported for `ToolUserInput` / `FileChange`; renderer currently responds with automatic empty answers, decline, and `"renderer tool UI is not implemented"`.

### Non-Goals

- Do not rename the native protocol or claim full Codex product parity.
- Do not build a full patch apply engine in this R1 plan. The UI collects and sends file-change approval decisions; actual file mutation remains owned by the runtime/tool layer.
- Do not add new dependencies. Use existing React, assistant-ui, Radix/shadcn-style UI primitives already present in `desktop-app`.

## File Structure

- Modify `crates/dasclaw_app_server/src/lib.rs`
  - Add bridge-owned pending dynamic tool response storage.
  - Add a real client dynamic tool executor path for `open_url` / `client.open_url`.
  - Keep fail-safe behavior for unsupported response kinds.
  - Add regression tests around dynamic client tool response delivery.
- Modify `desktop-app/src/shared/appServerApi.ts`
  - Replace broad `Record<string, unknown>` server request params with method-specific request/response types.
- Create `desktop-app/src/renderer/src/lib/clientTools.ts`
  - Minimal renderer client tool executor for `open_url` / `client.open_url`.
  - Validate URL scheme before opening.
  - Return method-specific `item/tool/call` responses.
- Create `desktop-app/src/renderer/src/lib/clientTools.test.ts`
  - Unit tests for supported, unsupported, and unsafe URL tool calls.
- Create `desktop-app/src/renderer/src/lib/serverRequests.ts`
  - Queue reducer helpers and method-specific fail-safe response builders.
- Create `desktop-app/src/renderer/src/lib/serverRequests.test.ts`
  - Unit tests proving known R1 requests queue for UI and unknown requests fail closed.
- Modify `desktop-app/src/renderer/src/hooks/useDasclawAssistantRuntime.ts`
  - Return pending server requests and responder functions to `App`.
  - Stop auto-answering known R1 requests.
- Create `desktop-app/src/renderer/src/components/assistant-ui/server-request-panel.tsx`
  - Render pending approval/user-input/file-change/tool requests.
  - Submit explicit user decisions or run the minimal client tool executor.
- Modify `desktop-app/src/renderer/src/App.tsx`
  - Mount the server request panel inside the existing assistant runtime shell.
- Modify `desktop-app/src/renderer/src/lib/assistantMessages.test.ts`
  - Replace old auto-fail-closed assertions with queue-and-respond assertions.
- Modify `desktop-app/src/main/appServerManager.test.ts`
  - Keep forwarding/fail-closed main process coverage; add one assertion that known R1 methods are not main-process auto-rejected.
- Modify `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md`
  - Move R1 from “control-plane complete, product incomplete” to “end-to-end product complete” only after all tasks pass.

---

## Task 1: Type R1 Server Requests In The Desktop Shared API

**Files:**
- Modify: `/Users/nallylin/Documents/code/x-claw/desktop-app/src/shared/appServerApi.ts`
- Test: `/Users/nallylin/Documents/code/x-claw/desktop-app/src/renderer/src/lib/serverRequests.test.ts`
- Create: `/Users/nallylin/Documents/code/x-claw/desktop-app/src/renderer/src/lib/serverRequests.ts`

- [ ] **Step 1: Write failing tests for method-specific server request parsing**

Create `/Users/nallylin/Documents/code/x-claw/desktop-app/src/renderer/src/lib/serverRequests.test.ts`:

```ts
import { describe, expect, it } from 'vitest'

import {
  failClosedServerRequestResponse,
  queueServerRequest,
  removeServerRequest
} from './serverRequests'
import type { AppServerServerRequest } from '../../../shared/appServerApi'

describe('server request queue helpers', () => {
  it('queues a known tool user-input request without answering it', () => {
    const request: AppServerServerRequest = {
      hostId: 'host_1',
      requestId: 'input_1',
      method: 'item/tool/requestUserInput',
      params: {
        threadId: 'thread_1',
        turnId: 'turn_1',
        itemId: 'item_1',
        questions: [
          {
            id: 'mode',
            header: 'Mode',
            question: 'Pick a mode',
            isOther: false,
            isSecret: false,
            options: [{ label: 'Fast', description: 'Use the fast mode' }]
          }
        ]
      }
    }

    const queued = queueServerRequest([], request)

    expect(queued).toHaveLength(1)
    expect(queued[0]?.method).toBe('item/tool/requestUserInput')
    expect(queued[0]?.requestId).toBe('input_1')
  })

  it('replaces an existing request with the same host and request id', () => {
    const first: AppServerServerRequest = {
      hostId: 'host_1',
      requestId: 'approval_1',
      method: 'item/fileChange/requestApproval',
      params: {
        threadId: 'thread_1',
        turnId: 'turn_1',
        itemId: 'file_1',
        reason: 'apply patch',
        grantRoot: '/workspace'
      }
    }
    const second: AppServerServerRequest = {
      ...first,
      params: { ...first.params, reason: 'updated patch reason' }
    }

    const queued = queueServerRequest(queueServerRequest([], first), second)

    expect(queued).toHaveLength(1)
    expect(queued[0]?.params.reason).toBe('updated patch reason')
  })

  it('removes a request after a response is sent', () => {
    const request: AppServerServerRequest = {
      hostId: 'host_1',
      requestId: 'tool_1',
      method: 'item/tool/call',
      params: {
        threadId: 'thread_1',
        turnId: 'turn_1',
        callId: 'call_1',
        namespace: 'client',
        tool: 'open_url',
        arguments: { url: 'https://example.test' }
      }
    }

    const queued = removeServerRequest(queueServerRequest([], request), request)

    expect(queued).toEqual([])
  })

  it('builds method-specific fail-closed responses', () => {
    expect(failClosedServerRequestResponse('item/tool/requestUserInput')).toEqual({
      answers: {}
    })
    expect(failClosedServerRequestResponse('item/fileChange/requestApproval')).toEqual({
      decision: 'decline'
    })
    expect(failClosedServerRequestResponse('item/permissions/requestApproval')).toEqual({
      permissions: {},
      scope: 'turn',
      strictAutoReview: true
    })
    expect(failClosedServerRequestResponse('item/tool/call')).toEqual({
      contentItems: [{ type: 'inputText', text: 'client tool execution was not approved' }],
      success: false
    })
  })
})
```

- [ ] **Step 2: Run the new test and verify it fails**

Run:

```bash
cd /Users/nallylin/Documents/code/x-claw/desktop-app
npm test -- --run src/renderer/src/lib/serverRequests.test.ts
```

Expected: FAIL because `src/renderer/src/lib/serverRequests.ts` does not exist.

- [ ] **Step 3: Add method-specific shared request types**

Modify `/Users/nallylin/Documents/code/x-claw/desktop-app/src/shared/appServerApi.ts` so the server request section reads:

```ts
export type AppServerServerRequestMethod =
  | 'item/commandExecution/requestApproval'
  | 'item/permissions/requestApproval'
  | 'item/fileChange/requestApproval'
  | 'item/tool/requestUserInput'
  | 'item/tool/call'

export type AppServerCommandApprovalParams = {
  toolCallId?: string
  toolName?: string
  command?: string
  description?: string
  displayParameters?: unknown
  allowAlways?: boolean
}

export type AppServerPermissionsApprovalParams = {
  threadId: string
  turnId: string
  itemId: string
  cwd: string
  reason?: string
  permissions: unknown
}

export type AppServerFileChangeApprovalParams = {
  threadId: string
  turnId: string
  itemId: string
  reason?: string
  grantRoot?: string
}

export type AppServerToolUserInputQuestionOption = {
  label: string
  description: string
}

export type AppServerToolUserInputQuestion = {
  id: string
  header: string
  question: string
  isOther?: boolean
  isSecret?: boolean
  options?: AppServerToolUserInputQuestionOption[]
}

export type AppServerToolUserInputParams = {
  threadId: string
  turnId: string
  itemId: string
  questions: AppServerToolUserInputQuestion[]
}

export type AppServerDynamicToolCallParams = {
  threadId: string
  turnId: string
  callId: string
  namespace?: string
  tool: string
  arguments: unknown
}

export type AppServerServerRequestParamsByMethod = {
  'item/commandExecution/requestApproval': AppServerCommandApprovalParams
  'item/permissions/requestApproval': AppServerPermissionsApprovalParams
  'item/fileChange/requestApproval': AppServerFileChangeApprovalParams
  'item/tool/requestUserInput': AppServerToolUserInputParams
  'item/tool/call': AppServerDynamicToolCallParams
}

export type AppServerServerRequest<
  Method extends AppServerServerRequestMethod = AppServerServerRequestMethod
> = {
  [M in Method]: {
    requestId: string | number
    hostId: string
    method: M
    params: AppServerServerRequestParamsByMethod[M]
  }
}[Method]

export type AppServerApprovalRequest = AppServerServerRequest<
  'item/commandExecution/requestApproval' | 'item/permissions/requestApproval'
>

export type AppServerToolUserInputResponse = {
  answers: Record<string, { answers: string[] }>
}

export type AppServerDynamicToolCallResponse = {
  contentItems: Array<
    { type: 'inputText'; text: string } | { type: 'inputImage'; imageUrl: string }
  >
  success: boolean
}

export type AppServerServerRequestResponse =
  | { decision: AppServerApprovalDecision }
  | { decision: 'accept' | 'acceptForSession' | 'decline' | 'cancel' }
  | AppServerDynamicToolCallResponse
  | AppServerToolUserInputResponse
  | { permissions: unknown; scope?: 'turn' | 'session'; strictAutoReview?: boolean }
```

- [ ] **Step 4: Add queue helpers**

Create `/Users/nallylin/Documents/code/x-claw/desktop-app/src/renderer/src/lib/serverRequests.ts`:

```ts
import type {
  AppServerServerRequest,
  AppServerServerRequestMethod,
  AppServerServerRequestResponse
} from '../../../shared/appServerApi'

export function queueServerRequest(
  current: readonly AppServerServerRequest[],
  request: AppServerServerRequest
): AppServerServerRequest[] {
  return [
    ...current.filter(
      (item) => item.hostId !== request.hostId || item.requestId !== request.requestId
    ),
    request
  ]
}

export function removeServerRequest(
  current: readonly AppServerServerRequest[],
  request: Pick<AppServerServerRequest, 'hostId' | 'requestId'>
): AppServerServerRequest[] {
  return current.filter(
    (item) => item.hostId !== request.hostId || item.requestId !== request.requestId
  )
}

export function failClosedServerRequestResponse(
  method: AppServerServerRequestMethod
): AppServerServerRequestResponse {
  if (method === 'item/tool/call') {
    return {
      contentItems: [{ type: 'inputText', text: 'client tool execution was not approved' }],
      success: false
    }
  }
  if (method === 'item/tool/requestUserInput') {
    return { answers: {} }
  }
  if (method === 'item/fileChange/requestApproval') {
    return { decision: 'decline' }
  }
  if (method === 'item/permissions/requestApproval') {
    return { permissions: {}, scope: 'turn', strictAutoReview: true }
  }
  return {
    decision: {
      kind: 'reject',
      data: { reason: 'approval request was not approved in the renderer' }
    }
  }
}
```

- [ ] **Step 5: Run tests and commit**

Run:

```bash
cd /Users/nallylin/Documents/code/x-claw/desktop-app
npm test -- --run src/renderer/src/lib/serverRequests.test.ts
```

Expected: PASS.

Commit:

```bash
cd /Users/nallylin/Documents/code/x-claw
git add desktop-app/src/shared/appServerApi.ts desktop-app/src/renderer/src/lib/serverRequests.ts desktop-app/src/renderer/src/lib/serverRequests.test.ts
git commit -m $'feat: type r1 desktop server requests\n\n已检查 R1 desktop server request typing 是否已有，结论：已有宽泛 union 和 fail-closed 占位，缺少 method-specific renderer queue 类型。'
```

---

## Task 2: Add A Minimal Renderer Client Tool Executor

**Files:**
- Create: `/Users/nallylin/Documents/code/x-claw/desktop-app/src/renderer/src/lib/clientTools.ts`
- Test: `/Users/nallylin/Documents/code/x-claw/desktop-app/src/renderer/src/lib/clientTools.test.ts`

- [ ] **Step 1: Write failing tests for supported and rejected client tools**

Create `/Users/nallylin/Documents/code/x-claw/desktop-app/src/renderer/src/lib/clientTools.test.ts`:

```ts
import { describe, expect, it, vi } from 'vitest'

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
  it('opens http URLs for open_url', async () => {
    const open = vi.fn()

    const response = await runClientToolRequest(toolRequest('open_url', {
      url: 'https://example.test/page'
    }), { openExternal: open })

    expect(open).toHaveBeenCalledWith('https://example.test/page')
    expect(response).toEqual({
      contentItems: [{ type: 'inputText', text: 'Opened URL: https://example.test/page' }],
      success: true
    })
  })

  it('opens http URLs for client.open_url', async () => {
    const open = vi.fn()

    const response = await runClientToolRequest(toolRequest('client.open_url', {
      url: 'http://example.test/page'
    }), { openExternal: open })

    expect(open).toHaveBeenCalledWith('http://example.test/page')
    expect(response.success).toBe(true)
  })

  it('rejects unsafe URL schemes', async () => {
    const open = vi.fn()

    const response = await runClientToolRequest(toolRequest('open_url', {
      url: 'javascript:alert(1)'
    }), { openExternal: open })

    expect(open).not.toHaveBeenCalled()
    expect(response).toEqual({
      contentItems: [{ type: 'inputText', text: 'Rejected unsafe URL: javascript:alert(1)' }],
      success: false
    })
  })

  it('returns a tool error for unsupported client tools', async () => {
    const response = await runClientToolRequest(toolRequest('client.take_screenshot', {}), {
      openExternal: vi.fn()
    })

    expect(response).toEqual({
      contentItems: [{ type: 'inputText', text: 'Unsupported client tool: client.take_screenshot' }],
      success: false
    })
  })
})
```

- [ ] **Step 2: Run the new test and verify it fails**

Run:

```bash
cd /Users/nallylin/Documents/code/x-claw/desktop-app
npm test -- --run src/renderer/src/lib/clientTools.test.ts
```

Expected: FAIL because `src/renderer/src/lib/clientTools.ts` does not exist.

- [ ] **Step 3: Implement the minimal executor**

Create `/Users/nallylin/Documents/code/x-claw/desktop-app/src/renderer/src/lib/clientTools.ts`:

```ts
import type {
  AppServerDynamicToolCallResponse,
  AppServerServerRequest
} from '../../../shared/appServerApi'

type ClientToolRuntime = {
  openExternal: (url: string) => void | Promise<void>
}

export async function runClientToolRequest(
  request: AppServerServerRequest<'item/tool/call'>,
  runtime: ClientToolRuntime = { openExternal: (url) => window.open(url, '_blank', 'noopener') }
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
    return toolError(`Rejected unsafe URL: ${url}`)
  }

  await runtime.openExternal(url)
  return {
    contentItems: [{ type: 'inputText', text: `Opened URL: ${url}` }],
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
```

- [ ] **Step 4: Run tests and commit**

Run:

```bash
cd /Users/nallylin/Documents/code/x-claw/desktop-app
npm test -- --run src/renderer/src/lib/clientTools.test.ts
```

Expected: PASS.

Commit:

```bash
cd /Users/nallylin/Documents/code/x-claw
git add desktop-app/src/renderer/src/lib/clientTools.ts desktop-app/src/renderer/src/lib/clientTools.test.ts
git commit -m $'feat: add minimal r1 client tool executor\n\n已检查 R1 client tool executor 是否已有，结论：已有自动失败占位，缺少 open_url/client.open_url 的真实 renderer executor。'
```

---

## Task 3: Queue Known R1 Server Requests In The Renderer Hook

**Files:**
- Modify: `/Users/nallylin/Documents/code/x-claw/desktop-app/src/renderer/src/hooks/useDasclawAssistantRuntime.ts`
- Modify: `/Users/nallylin/Documents/code/x-claw/desktop-app/src/renderer/src/lib/assistantMessages.test.ts`

- [ ] **Step 1: Replace the old auto-fail-closed test with queue behavior**

In `/Users/nallylin/Documents/code/x-claw/desktop-app/src/renderer/src/lib/assistantMessages.test.ts`, replace the old test that expects immediate fail-closed responses for R1 methods with:

```ts
it('queues known R1 server requests for renderer UI instead of auto-answering them', async () => {
  renderHook(() => useDasclawAssistantRuntime())

  const listener = notificationListeners.at(-1)
  expect(listener).toBeDefined()
  listener?.({
    hostId: 'host_1',
    requestId: 'input_1',
    method: 'item/tool/requestUserInput',
    params: {
      threadId: 'thread_1',
      turnId: 'turn_1',
      itemId: 'item_1',
      questions: [
        {
          id: 'mode',
          header: 'Mode',
          question: 'Pick a mode',
          isOther: false,
          isSecret: false
        }
      ]
    }
  })

  await waitFor(() => {
    expect(respondServerRequestMock).not.toHaveBeenCalled()
  })
})
```

Add a second test that verifies explicit response removes the queued request:

```ts
it('responds to a queued server request only after explicit renderer action', async () => {
  const { result } = renderHook(() => useDasclawAssistantRuntime())
  const listener = notificationListeners.at(-1)

  listener?.({
    hostId: 'host_1',
    requestId: 'file_1',
    method: 'item/fileChange/requestApproval',
    params: {
      threadId: 'thread_1',
      turnId: 'turn_1',
      itemId: 'file_1',
      reason: 'apply patch',
      grantRoot: '/workspace'
    }
  })

  await waitFor(() => {
    expect(result.current.serverRequests).toHaveLength(1)
  })

  await act(async () => {
    await result.current.respondToServerRequest(result.current.serverRequests[0], {
      decision: 'accept'
    })
  })

  expect(respondServerRequestMock).toHaveBeenCalledWith('file_1', { decision: 'accept' })
  expect(result.current.serverRequests).toEqual([])
})
```

- [ ] **Step 2: Run the modified tests and verify they fail**

Run:

```bash
cd /Users/nallylin/Documents/code/x-claw/desktop-app
npm test -- --run src/renderer/src/lib/assistantMessages.test.ts
```

Expected: FAIL because `useDasclawAssistantRuntime()` does not expose `serverRequests` or `respondToServerRequest`.

- [ ] **Step 3: Modify the hook return type and notification handling**

In `/Users/nallylin/Documents/code/x-claw/desktop-app/src/renderer/src/hooks/useDasclawAssistantRuntime.ts`, update imports:

```ts
import {
  failClosedServerRequestResponse,
  queueServerRequest,
  removeServerRequest
} from '../lib/serverRequests'
```

Update `DasclawAssistantRuntime`:

```ts
type DasclawAssistantRuntime = {
  runtime: AssistantRuntime
  status: AppServerStatus | undefined
  serverRequests: readonly AppServerServerRequest[]
  respondToServerRequest: (
    request: AppServerServerRequest,
    response: AppServerServerRequestResponse
  ) => Promise<void>
  rejectServerRequest: (request: AppServerServerRequest) => Promise<void>
}
```

Inside `useDasclawAssistantRuntime()`, add state and response callbacks:

```ts
const [serverRequests, setServerRequests] = useState<AppServerServerRequest[]>([])

const respondToServerRequest = useCallback(
  async (
    request: AppServerServerRequest,
    response: AppServerServerRequestResponse
  ): Promise<void> => {
    await window.desktopAppServer.respondServerRequest(request.requestId, response)
    setServerRequests((current) => removeServerRequest(current, request))
  },
  []
)

const rejectServerRequest = useCallback(
  async (request: AppServerServerRequest): Promise<void> => {
    await respondToServerRequest(request, failClosedServerRequestResponse(request.method))
  },
  [respondToServerRequest]
)
```

Replace the server request branch in the notification listener:

```ts
if (isServerRequest(notification)) {
  setServerRequests((current) => queueServerRequest(current, notification))
  return
}
```

Remove the local `rejectServerRequestUntilUiExists()` and `failClosedRendererServerRequestResponse()` functions from this file.

Return the new values:

```ts
return {
  runtime,
  status,
  serverRequests,
  respondToServerRequest,
  rejectServerRequest
}
```

- [ ] **Step 4: Run hook tests and commit**

Run:

```bash
cd /Users/nallylin/Documents/code/x-claw/desktop-app
npm test -- --run src/renderer/src/lib/assistantMessages.test.ts src/renderer/src/lib/serverRequests.test.ts
```

Expected: PASS.

Commit:

```bash
cd /Users/nallylin/Documents/code/x-claw
git add desktop-app/src/renderer/src/hooks/useDasclawAssistantRuntime.ts desktop-app/src/renderer/src/lib/assistantMessages.test.ts
git commit -m $'feat: queue r1 renderer server requests\n\n已检查 R1 renderer request UI 是否已有，结论：已有自动 fail-closed 占位，缺少显式队列和用户触发响应。'
```

---

## Task 4: Build The Renderer Server Request Panel

**Files:**
- Create: `/Users/nallylin/Documents/code/x-claw/desktop-app/src/renderer/src/components/assistant-ui/server-request-panel.tsx`
- Modify: `/Users/nallylin/Documents/code/x-claw/desktop-app/src/renderer/src/App.tsx`
- Test: `/Users/nallylin/Documents/code/x-claw/desktop-app/src/renderer/src/App.test.tsx`

- [ ] **Step 1: Add a component test for visible queued requests**

In `/Users/nallylin/Documents/code/x-claw/desktop-app/src/renderer/src/App.test.tsx`, add:

```tsx
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'

import { ServerRequestPanel } from './components/assistant-ui/server-request-panel'
import type { AppServerServerRequest } from '../../shared/appServerApi'

describe('ServerRequestPanel', () => {
  it('submits a file-change approval decision', async () => {
    const onRespond = vi.fn().mockResolvedValue(undefined)
    const onReject = vi.fn().mockResolvedValue(undefined)
    const requests: AppServerServerRequest[] = [
      {
        hostId: 'host_1',
        requestId: 'file_1',
        method: 'item/fileChange/requestApproval',
        params: {
          threadId: 'thread_1',
          turnId: 'turn_1',
          itemId: 'file_1',
          reason: 'apply patch',
          grantRoot: '/workspace'
        }
      }
    ]

    render(
      <ServerRequestPanel requests={requests} onRespond={onRespond} onReject={onReject} />
    )

    await userEvent.click(screen.getByRole('button', { name: 'Accept file changes' }))

    expect(onRespond).toHaveBeenCalledWith(requests[0], { decision: 'accept' })
  })
})
```

- [ ] **Step 2: Run the component test and verify it fails**

Run:

```bash
cd /Users/nallylin/Documents/code/x-claw/desktop-app
npm test -- --run src/renderer/src/App.test.tsx
```

Expected: FAIL because `server-request-panel.tsx` does not exist.

- [ ] **Step 3: Implement the panel**

Create `/Users/nallylin/Documents/code/x-claw/desktop-app/src/renderer/src/components/assistant-ui/server-request-panel.tsx`:

```tsx
import { useMemo, useState } from 'react'
import { ShieldCheckIcon, ShieldXIcon, WrenchIcon } from 'lucide-react'

import type {
  AppServerServerRequest,
  AppServerServerRequestResponse
} from '../../../../shared/appServerApi'
import { runClientToolRequest } from '../../lib/clientTools'
import { Button } from '../ui/button'

type ServerRequestPanelProps = {
  requests: readonly AppServerServerRequest[]
  onRespond: (
    request: AppServerServerRequest,
    response: AppServerServerRequestResponse
  ) => Promise<void>
  onReject: (request: AppServerServerRequest) => Promise<void>
}

export function ServerRequestPanel({
  requests,
  onRespond,
  onReject
}: ServerRequestPanelProps): React.JSX.Element | null {
  const [busyRequestId, setBusyRequestId] = useState<string | number>()
  const active = requests[0]
  const title = useMemo(() => (active ? titleForRequest(active) : ''), [active])

  if (!active) return null

  const submit = async (response: AppServerServerRequestResponse): Promise<void> => {
    setBusyRequestId(active.requestId)
    try {
      await onRespond(active, response)
    } finally {
      setBusyRequestId(undefined)
    }
  }

  const reject = async (): Promise<void> => {
    setBusyRequestId(active.requestId)
    try {
      await onReject(active)
    } finally {
      setBusyRequestId(undefined)
    }
  }

  const disabled = busyRequestId === active.requestId

  return (
    <section className="border-t border-border bg-background px-4 py-3">
      <div className="mx-auto flex w-full max-w-(--thread-max-width) flex-col gap-3">
        <div className="flex items-center gap-2 text-sm font-medium">
          <WrenchIcon className="size-4" />
          <span>{title}</span>
        </div>
        <RequestBody request={active} onSubmit={submit} disabled={disabled} />
        <div className="flex items-center justify-end gap-2">
          <Button variant="outline" size="sm" disabled={disabled} onClick={reject}>
            <ShieldXIcon className="size-4" />
            Reject
          </Button>
        </div>
      </div>
    </section>
  )
}

function RequestBody({
  request,
  onSubmit,
  disabled
}: {
  request: AppServerServerRequest
  onSubmit: (response: AppServerServerRequestResponse) => Promise<void>
  disabled: boolean
}): React.JSX.Element {
  if (request.method === 'item/fileChange/requestApproval') {
    return (
      <div className="flex flex-col gap-2 text-sm">
        <p className="text-muted-foreground">{request.params.reason ?? 'File change requested'}</p>
        {request.params.grantRoot ? (
          <code className="rounded bg-muted px-2 py-1 text-xs">{request.params.grantRoot}</code>
        ) : null}
        <div className="flex justify-end gap-2">
          <Button
            size="sm"
            disabled={disabled}
            onClick={() => onSubmit({ decision: 'accept' })}
          >
            <ShieldCheckIcon className="size-4" />
            Accept file changes
          </Button>
          <Button
            size="sm"
            variant="outline"
            disabled={disabled}
            onClick={() => onSubmit({ decision: 'acceptForSession' })}
          >
            <ShieldCheckIcon className="size-4" />
            Accept for session
          </Button>
        </div>
      </div>
    )
  }

  if (request.method === 'item/permissions/requestApproval') {
    return (
      <div className="flex flex-col gap-2 text-sm">
        <p className="text-muted-foreground">{request.params.reason ?? 'Permission requested'}</p>
        <pre className="max-h-36 overflow-auto rounded bg-muted p-2 text-xs">
          {JSON.stringify(request.params.permissions, null, 2)}
        </pre>
        <div className="flex justify-end gap-2">
          <Button
            size="sm"
            disabled={disabled}
            onClick={() =>
              onSubmit({
                permissions: request.params.permissions,
                scope: 'turn',
                strictAutoReview: true
              })
            }
          >
            <ShieldCheckIcon className="size-4" />
            Allow once
          </Button>
          <Button
            size="sm"
            variant="outline"
            disabled={disabled}
            onClick={() =>
              onSubmit({
                permissions: request.params.permissions,
                scope: 'session',
                strictAutoReview: true
              })
            }
          >
            <ShieldCheckIcon className="size-4" />
            Allow session
          </Button>
        </div>
      </div>
    )
  }

  if (request.method === 'item/tool/requestUserInput') {
    return <UserInputRequest request={request} onSubmit={onSubmit} disabled={disabled} />
  }

  if (request.method === 'item/tool/call') {
    return (
      <div className="flex flex-col gap-2 text-sm">
        <p className="text-muted-foreground">Client tool: {request.params.tool}</p>
        <pre className="max-h-36 overflow-auto rounded bg-muted p-2 text-xs">
          {JSON.stringify(request.params.arguments, null, 2)}
        </pre>
        <div className="flex justify-end">
          <Button
            size="sm"
            disabled={disabled}
            onClick={async () => onSubmit(await runClientToolRequest(request))}
          >
            <WrenchIcon className="size-4" />
            Run client tool
          </Button>
        </div>
      </div>
    )
  }

  return (
    <div className="text-sm text-muted-foreground">
      Command approval request received.
    </div>
  )
}

function UserInputRequest({
  request,
  onSubmit,
  disabled
}: {
  request: AppServerServerRequest<'item/tool/requestUserInput'>
  onSubmit: (response: AppServerServerRequestResponse) => Promise<void>
  disabled: boolean
}): React.JSX.Element {
  const [answers, setAnswers] = useState<Record<string, string>>({})

  return (
    <form
      className="flex flex-col gap-3 text-sm"
      onSubmit={(event) => {
        event.preventDefault()
        void onSubmit({
          answers: Object.fromEntries(
            request.params.questions.map((question) => [
              question.id,
              { answers: [answers[question.id] ?? ''] }
            ])
          )
        })
      }}
    >
      {request.params.questions.map((question) => (
        <label key={question.id} className="flex flex-col gap-1">
          <span className="font-medium">{question.header}</span>
          <span className="text-muted-foreground">{question.question}</span>
          <input
            className="h-9 rounded-md border border-input bg-background px-3 text-sm"
            type={question.isSecret ? 'password' : 'text'}
            value={answers[question.id] ?? ''}
            onChange={(event) =>
              setAnswers((current) => ({ ...current, [question.id]: event.target.value }))
            }
          />
        </label>
      ))}
      <div className="flex justify-end">
        <Button size="sm" disabled={disabled} type="submit">
          <ShieldCheckIcon className="size-4" />
          Submit answers
        </Button>
      </div>
    </form>
  )
}

function titleForRequest(request: AppServerServerRequest): string {
  if (request.method === 'item/fileChange/requestApproval') return 'Review file changes'
  if (request.method === 'item/permissions/requestApproval') return 'Review permissions'
  if (request.method === 'item/tool/requestUserInput') return 'Answer tool questions'
  if (request.method === 'item/tool/call') return 'Run client tool'
  return 'Review command approval'
}
```

- [ ] **Step 4: Mount the panel**

In `/Users/nallylin/Documents/code/x-claw/desktop-app/src/renderer/src/App.tsx`, update the hook call:

```tsx
const { runtime, serverRequests, respondToServerRequest, rejectServerRequest } =
  useDasclawAssistantRuntime()
```

Import the component:

```tsx
import { ServerRequestPanel } from './components/assistant-ui/server-request-panel'
```

Inside the main chat card, render the panel below `ChatThread`:

```tsx
<div className="min-h-0 flex-1 overflow-hidden">
  <ChatThread />
</div>
<ServerRequestPanel
  requests={serverRequests}
  onRespond={respondToServerRequest}
  onReject={rejectServerRequest}
/>
```

- [ ] **Step 5: Run renderer tests and commit**

Run:

```bash
cd /Users/nallylin/Documents/code/x-claw/desktop-app
npm test -- --run src/renderer/src/App.test.tsx src/renderer/src/lib/clientTools.test.ts src/renderer/src/lib/assistantMessages.test.ts
npm run typecheck:web
```

Expected: all commands PASS.

Commit:

```bash
cd /Users/nallylin/Documents/code/x-claw
git add desktop-app/src/renderer/src/App.tsx desktop-app/src/renderer/src/App.test.tsx desktop-app/src/renderer/src/components/assistant-ui/server-request-panel.tsx
git commit -m $'feat: add r1 renderer server request UI\n\n已检查 R1 renderer UI 是否已有，结论：已有 assistant shell 和 fail-closed placeholder，缺少用户可操作的 request panel。'
```

---

## Task 5: Make Dynamic Client Tool Responses Reach The Runtime Bridge

**Files:**
- Modify: `/Users/nallylin/Documents/code/x-claw/crates/dasclaw_app_server/src/lib.rs`

- [ ] **Step 1: Write a failing Rust regression test**

In `/Users/nallylin/Documents/code/x-claw/crates/dasclaw_app_server/src/lib.rs`, add this test near `agent_runtime_bridge_maps_tool_events_to_r1_surface`:

```rust
#[test]
fn agent_runtime_bridge_waits_for_client_dynamic_tool_response() {
    let responder = Arc::new(ScriptedResponder::new(vec![
        client_tool_call_output("open_url", "call_1"),
        text_output("done after client tool"),
    ]));
    let bridge = DasclawAgentRuntimeBridge::from_responder(responder);
    let (updates, rx) = RuntimeTurnUpdateSink::channel_for_test();

    bridge
        .start_turn(RuntimeTurnStartRequest {
            thread_id: "thread_1".to_string(),
            turn_id: "turn_1".to_string(),
            prompt: "open example".to_string(),
            cwd: PathBuf::from("/workspace"),
            sandbox_context: RuntimeSandboxContext::default(),
            model_provider: RuntimeModelProviderSnapshot::for_test(),
            reasoning_summary: ReasoningSummary::default(),
            updates,
        })
        .expect("turn starts");

    let request = wait_for_runtime_update(&rx, |update| match update.outcome {
        RuntimeTurnOutcome::DynamicToolCallRequested { request } => Some(request),
        _ => None,
    })
    .expect("dynamic tool request emitted");

    assert_eq!(request.request_id, "call_1");

    bridge
        .resolve_server_request(RuntimeServerRequestResolution {
            request_id: "call_1".to_string(),
            payload: RuntimeServerRequestResponse::DynamicTool(DynamicToolCallResponse {
                content_items: vec![DynamicToolCallOutputContentItem::InputText {
                    text: "opened".to_string(),
                }],
                success: true,
            }),
        })
        .expect("dynamic tool response resolves");

    let result = wait_for_runtime_update(&rx, |update| match update.outcome {
        RuntimeTurnOutcome::ToolResult { update } => Some(update),
        _ => None,
    })
    .expect("tool result emitted after client response");

    assert_eq!(result.item_id, "turn_1:tool:call_1");
    assert_eq!(result.content, "opened");
    assert!(!result.is_error);

    wait_for_runtime_update(&rx, |update| match update.outcome {
        RuntimeTurnOutcome::Completed { output } => Some(output),
        _ => None,
    })
    .expect("turn completes after dynamic client tool result");
}
```

- [ ] **Step 2: Run the test and verify it fails**

Run:

```bash
cd /Users/nallylin/Documents/code/x-claw
RUSTC_WRAPPER= cargo nextest run -p dasclaw_app_server -E 'test(agent_runtime_bridge_waits_for_client_dynamic_tool_response)'
```

Expected: FAIL because `DynamicTool` responses are currently accepted as `Ok(())` but are not delivered to the agent's tool execution path.

- [ ] **Step 3: Add bridge-owned pending dynamic tool responses**

Modify `DasclawAgentRuntimeBridge` in `/Users/nallylin/Documents/code/x-claw/crates/dasclaw_app_server/src/lib.rs`:

```rust
#[derive(Clone)]
pub struct DasclawAgentRuntimeBridge {
    agent_factory: Arc<AgentFactory>,
    in_flight: Arc<Mutex<HashMap<String, CancellationToken>>>,
    active_agents: Arc<Mutex<HashMap<String, Arc<dasclaw_runtime::Agent>>>>,
    pending_approvals: Arc<Mutex<HashMap<String, Arc<dasclaw_runtime::Agent>>>>,
    pending_dynamic_tools:
        Arc<Mutex<HashMap<String, std::sync::mpsc::Sender<DynamicToolCallResponse>>>>,
    features: RuntimeBridgeFeatures,
}
```

Initialize it in `new_with_model_provider`:

```rust
pending_dynamic_tools: Arc::new(Mutex::new(HashMap::new())),
```

Add a helper:

```rust
fn take_pending_dynamic_tool(
    &self,
    request_id: &str,
) -> Result<std::sync::mpsc::Sender<DynamicToolCallResponse>, RuntimeBridgeError> {
    self.pending_dynamic_tools
        .lock()
        .map_err(|_| RuntimeBridgeError::retryable("runtime dynamic tool registry lock poisoned"))?
        .remove(request_id)
        .ok_or_else(|| {
            RuntimeBridgeError::retryable(format!(
                "runtime dynamic tool request is not pending: {request_id}"
            ))
        })
}
```

Change `resolve_server_request`:

```rust
RuntimeServerRequestResponse::DynamicTool(response) => {
    let sender = self.take_pending_dynamic_tool(&resolution.request_id)?;
    sender
        .send(response)
        .map_err(|_| RuntimeBridgeError::retryable("runtime dynamic tool receiver dropped"))
}
```

- [ ] **Step 4: Add a client dynamic tool executor**

Add this struct near the other runtime bridge helpers:

```rust
#[derive(Debug, Clone)]
struct RuntimeClientDynamicToolExecutor {
    thread_id: String,
    turn_id: String,
    updates: RuntimeTurnUpdateSink,
    pending_dynamic_tools:
        Arc<Mutex<HashMap<String, std::sync::mpsc::Sender<DynamicToolCallResponse>>>>,
}

#[async_trait::async_trait]
impl dasclaw_runtime::ToolExecutor for RuntimeClientDynamicToolExecutor {
    async fn execute(&self, call: &ToolCall) -> Result<ToolResult, HostError> {
        if !is_client_dynamic_tool(&call.name) {
            return Ok(ToolResult {
                tool_call_id: call.id.clone(),
                name: call.name.clone(),
                content: format!("unsupported app-server client tool: {}", call.name),
                is_error: true,
            });
        }

        let (tx, rx) = std::sync::mpsc::channel();
        self.pending_dynamic_tools
            .lock()
            .map_err(|_| HostError::from("runtime dynamic tool registry lock poisoned"))?
            .insert(call.id.clone(), tx);

        self.updates.dynamic_tool_call_requested(
            self.thread_id.clone(),
            self.turn_id.clone(),
            RuntimeDynamicToolCallRequest {
                request_id: call.id.clone(),
                call_id: call.id.clone(),
                namespace: Some("client".to_string()),
                tool: call.name.clone(),
                arguments: call.arguments.clone(),
            },
        );

        let response = tokio::task::spawn_blocking(move || rx.recv())
            .await
            .map_err(|error| HostError::from(format!("dynamic tool wait failed: {error}")))?
            .map_err(|_| HostError::from("dynamic tool response channel closed"))?;

        Ok(ToolResult {
            tool_call_id: call.id.clone(),
            name: call.name.clone(),
            content: dynamic_tool_response_text(&response),
            is_error: !response.success,
        })
    }
}

fn dynamic_tool_response_text(response: &DynamicToolCallResponse) -> String {
    response
        .content_items
        .iter()
        .map(|item| match item {
            DynamicToolCallOutputContentItem::InputText { text } => text.clone(),
            DynamicToolCallOutputContentItem::InputImage { image_url } => image_url.clone(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}
```

In `start_turn`, before creating the agent, construct an executor for the turn and use a new bridge-owned factory path for `from_responder` tests and `from_model_provider_snapshot`. The final shape should ensure the `Agent` has `tool_executor_arc(Arc::new(RuntimeClientDynamicToolExecutor { ... }))` when no stronger executor is explicitly injected by tests.

- [ ] **Step 5: Remove duplicate ToolCallStart dynamic request emission**

In the `AgentEvent::ToolCallStart` branch, keep command output delta for non-client tools. For client tools, do not emit a second `DynamicToolCallRequested`; the executor now owns that request:

```rust
if !is_client_dynamic_tool(&name) {
    event_updates.command_output_delta(
        event_thread_id.clone(),
        event_turn_id.clone(),
        RuntimeCommandOutputDeltaUpdate {
            item_id: runtime_tool_item_id(&event_turn_id, &call_id),
            delta: tool_call_started_delta(&name),
        },
    );
}
```

- [ ] **Step 6: Run Rust tests and commit**

Run:

```bash
cd /Users/nallylin/Documents/code/x-claw
RUSTC_WRAPPER= cargo nextest run -p dasclaw_app_server -E 'test(agent_runtime_bridge_waits_for_client_dynamic_tool_response) | test(agent_runtime_bridge_maps_tool_events_to_r1_surface) | test(malformed_dynamic_tool_response_fails_turn_and_clears_pending_request)'
RUSTC_WRAPPER= cargo check -p dasclaw_app_server --tests
```

Expected: all commands PASS.

Commit:

```bash
cd /Users/nallylin/Documents/code/x-claw
git add crates/dasclaw_app_server/src/lib.rs
git commit -m $'feat: deliver r1 client tool responses to runtime\n\n已检查 R1 dynamic client tool response 是否已有，结论：已有 ServerRequest surface，但真实 bridge 对 DynamicTool response 只是 no-op，需要 pending response plumbing。'
```

---

## Task 6: Keep File/User-Input Unsupported Runtime Responses Honest

**Files:**
- Modify: `/Users/nallylin/Documents/code/x-claw/crates/dasclaw_app_server/src/lib.rs`
- Modify: `/Users/nallylin/Documents/code/x-claw/docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md`

- [ ] **Step 1: Add tests that guard the honest boundary**

Add this Rust test near the existing R1 bridge tests:

```rust
#[test]
fn real_runtime_bridge_rejects_file_and_user_input_resolution_until_runtime_owner_exists() {
    let bridge = DasclawAgentRuntimeBridge::from_responder(Arc::new(ScriptedResponder::new(vec![
        text_output("done"),
    ])));

    let user_input = bridge.resolve_server_request(RuntimeServerRequestResolution {
        request_id: "input_1".to_string(),
        payload: RuntimeServerRequestResponse::ToolUserInput(ToolRequestUserInputResponse {
            answers: HashMap::new(),
        }),
    });
    assert!(user_input.is_err());
    assert!(
        user_input
            .unwrap_err()
            .to_string()
            .contains("does not support this server request response kind")
    );

    let file_change = bridge.resolve_server_request(RuntimeServerRequestResolution {
        request_id: "file_1".to_string(),
        payload: RuntimeServerRequestResponse::FileChange(FileChangeApprovalDecision::Accept),
    });
    assert!(file_change.is_err());
    assert!(
        file_change
            .unwrap_err()
            .to_string()
            .contains("does not support this server request response kind")
    );
}
```

- [ ] **Step 2: Run the test**

Run:

```bash
cd /Users/nallylin/Documents/code/x-claw
RUSTC_WRAPPER= cargo nextest run -p dasclaw_app_server -E 'test(real_runtime_bridge_rejects_file_and_user_input_resolution_until_runtime_owner_exists)'
```

Expected: PASS. This locks the current honest boundary so product UI work cannot be confused with runtime file/user-input owner completion.

- [ ] **Step 3: Keep matrix wording precise**

Update `/Users/nallylin/Documents/code/x-claw/docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md` R1 row after Task 5:

```markdown
| R1 | Runtime tool / approval owner | ~~`item/tool/call` client `open_url` E2E~~、~~renderer-visible `item/tool/requestUserInput`~~、~~renderer-visible `item/permissions/requestApproval`~~、~~renderer-visible `item/fileChange/requestApproval`~~、~~renderer-visible file-change / auto-review notifications~~；runtime-owned file-change apply/user-input suspension 仍保持 explicit unsupported，直到底层 runtime producer 不再依赖测试桥 | app-server 已能把 client dynamic tool response 送回 runtime；desktop renderer 不再用自动 reject / 空 answers 冒充 UI；file-change/user-input 已有用户交互和 fail-safe response，但真实 runtime owner 仍需单独任务接入 | R1 本轮完成判定：client `open_url` 能从 LLM tool call 到 renderer 执行再回到 runtime tool result；renderer 对 user-input/file-change/permissions 给出显式用户交互；auto-review/file-change notification 可见；测试明确区分真实 runtime path 与 test bridge path |
```

- [ ] **Step 4: Commit**

Run:

```bash
cd /Users/nallylin/Documents/code/x-claw
git add crates/dasclaw_app_server/src/lib.rs docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md
git commit -m $'docs: clarify r1 product completion boundary\n\n已检查 R1 file-change/user-input runtime owner 是否已有，结论：renderer 产品交互可补实，真实 runtime owner 仍保持 explicit unsupported。'
```

---

## Task 7: Final Verification

**Files:**
- Verify all files changed by Tasks 1-6.

- [ ] **Step 1: Run focused Rust verification**

Run:

```bash
cd /Users/nallylin/Documents/code/x-claw
RUSTC_WRAPPER= cargo nextest run -p dasclaw_app_server -E 'test(runtime_features_gate_r1_capability_advertising) | test(r1_runtime_updates_emit_server_requests_and_notifications) | test(agent_runtime_bridge_waits_for_client_dynamic_tool_response) | test(agent_runtime_bridge_maps_tool_events_to_r1_surface) | test(agent_runtime_bridge_maps_non_command_approval_to_permissions_request) | test(real_runtime_bridge_rejects_file_and_user_input_resolution_until_runtime_owner_exists) | test(malformed_dynamic_tool_response_fails_turn_and_clears_pending_request)'
RUSTC_WRAPPER= cargo check -p dasclaw_app_server --tests
```

Expected: all selected nextest tests pass; cargo check exits with zero errors.

- [ ] **Step 2: Run focused desktop verification**

Run:

```bash
cd /Users/nallylin/Documents/code/x-claw/desktop-app
npm test -- --run src/main/appServerManager.test.ts src/renderer/src/App.test.tsx src/renderer/src/lib/assistantMessages.test.ts src/renderer/src/lib/serverRequests.test.ts src/renderer/src/lib/clientTools.test.ts
npm run typecheck
```

Expected: all Vitest files pass and both node/web TypeScript projects typecheck.

- [ ] **Step 3: Run formatting and panic guard**

Run:

```bash
cd /Users/nallylin/Documents/code/x-claw
cargo fmt --all
python3.12 scripts/check_no_panics.py --base origin/xClaw
```

Expected: both commands exit successfully.

- [ ] **Step 4: Review the final diff**

Run:

```bash
cd /Users/nallylin/Documents/code/x-claw
git diff --stat xClaw..HEAD
git diff -- docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md
```

Expected: diff shows the R1 product-completion plan implemented without unrelated rewrites; the matrix does not claim full file-change/user-input runtime owner completion.

- [ ] **Step 5: Commit final verification note if formatting changed files**

If `cargo fmt --all` changed files, commit them:

```bash
cd /Users/nallylin/Documents/code/x-claw
git add .
git commit -m $'chore: format r1 product completion changes\n\n已检查 R1 final verification 是否已有，结论：格式化只收敛本轮改动。'
```

---

## Self-Review

- **Spec coverage:** The plan covers the R1 remaining development boundary recorded in `dasclaw-app-server-codex-protocol-gap-matrix.md`: real renderer UI, client tool executor, dynamic client tool runtime response delivery, and explicit tests preventing automatic reject/empty answers from being counted as product completion.
- **Placeholder scan:** 禁用占位模式已检查，未发现问题。Each code-changing task includes exact files, concrete test code, implementation snippets, commands, expected results, and commit commands.
- **Type consistency:** `AppServerServerRequest`, `AppServerServerRequestResponse`, `serverRequests`, `respondToServerRequest`, `rejectServerRequest`, `runClientToolRequest`, and `ServerRequestPanel` names are consistent across tasks.
