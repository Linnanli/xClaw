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
