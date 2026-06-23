import type {
  AppServerServerRequest,
  AppServerServerRequestMethod,
  AppServerServerRequestResponseByMethod,
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

const failClosedResponses: AppServerServerRequestResponseByMethod = {
  'item/tool/call': {
    contentItems: [{ type: 'inputText', text: 'client tool execution was not approved' }],
    success: false
  },
  'item/tool/requestUserInput': { answers: {} },
  'item/fileChange/requestApproval': { decision: 'decline' },
  'item/permissions/requestApproval': {
    decision: 'reject',
    permissions: {},
    scope: 'turn',
    strictAutoReview: true
  },
  'item/commandExecution/requestApproval': {
    decision: {
      kind: 'reject',
      data: { reason: 'approval request was not approved in the renderer' }
    }
  }
}

export function failClosedServerRequestResponse<Method extends AppServerServerRequestMethod>(
  method: Method
): AppServerServerRequestResponse<Method> {
  return failClosedResponses[method]
}
