import { describe, expect, it } from 'vitest'

import {
  failClosedServerRequestResponse,
  queueServerRequest,
  removeServerRequest
} from './serverRequests'
import type {
  AppServerServerRequest,
  AppServerServerRequestResponse
} from '../../../shared/appServerApi'

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
    const queuedRequest = queued[0]

    expect(queued).toHaveLength(1)
    expect(queuedRequest?.method).toBe('item/fileChange/requestApproval')
    if (queuedRequest?.method !== 'item/fileChange/requestApproval') {
      throw new Error('expected a file change approval request')
    }
    expect(queuedRequest.params.reason).toBe('updated patch reason')
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
    const commandResponse = failClosedServerRequestResponse(
      'item/commandExecution/requestApproval'
    ) satisfies AppServerServerRequestResponse<'item/commandExecution/requestApproval'>

    expect(commandResponse).toEqual({
      decision: {
        kind: 'reject',
        data: { reason: 'approval request was not approved in the renderer' }
      }
    })
    expect(failClosedServerRequestResponse('item/tool/requestUserInput')).toEqual({
      answers: {}
    })
    expect(failClosedServerRequestResponse('item/fileChange/requestApproval')).toEqual({
      decision: 'decline'
    })
    expect(failClosedServerRequestResponse('item/permissions/requestApproval')).toEqual({
      decision: 'reject',
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
