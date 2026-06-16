import type {
  AppendMessage,
  ThreadAssistantMessagePart,
  ThreadAssistantMessage,
  ThreadMessage,
  ThreadUserMessage
} from '@assistant-ui/react'
import type { RendererModelProviderConfig } from '../../../shared/appServerApi'

export type AssistantModelOption = {
  id: string
  name: string
  description?: string
  disabled?: boolean
  keywords?: readonly string[]
}

export const defaultAssistantModelId = 'dasclaw-default'
export const pendingAssistantMessageText = '正在思考'

type PendingMessageContentPart = {
  readonly type: string
  readonly text?: string
}

export const assistantModelOptions: AssistantModelOption[] = [
  { id: defaultAssistantModelId, name: 'Dasclaw Default' },
  { id: 'dasclaw-fast', name: 'Dasclaw Fast' },
  { id: 'dasclaw-deep', name: 'Dasclaw Deep' }
]

export function modelOptionsFromProviderConfig(
  config: RendererModelProviderConfig
): AssistantModelOption[] {
  return config.models.map((model) => {
    const isConfigured = model.apiKeyConfigured
    return {
      id: model.modelId,
      name: model.displayName,
      description: model.description ?? (isConfigured ? undefined : 'API Key 未配置'),
      ...(isConfigured ? {} : { disabled: true }),
      keywords: [model.provider, model.source].filter(Boolean)
    }
  })
}

export function initialAssistantMessages(): ThreadMessage[] {
  return []
}

export function extractTextFromAppendMessage(message: AppendMessage): string {
  return message.content
    .filter((part): part is { type: 'text'; text: string } => part.type === 'text')
    .map((part) => part.text)
    .join('\n')
    .trim()
}

export function userMessage(id: string, text: string): ThreadUserMessage {
  return {
    id,
    role: 'user',
    createdAt: new Date(),
    content: [{ type: 'text', text }],
    attachments: [],
    metadata: {
      custom: {}
    }
  }
}

export function assistantMessage(
  id: string,
  text: string,
  status: ThreadAssistantMessage['status'] = { type: 'complete', reason: 'stop' }
): ThreadAssistantMessage {
  return assistantMessageWithContent(id, [{ type: 'text', text }], status)
}

export function assistantMessageWithContent(
  id: string,
  content: readonly ThreadAssistantMessagePart[],
  status: ThreadAssistantMessage['status'] = { type: 'complete', reason: 'stop' }
): ThreadAssistantMessage {
  return {
    id,
    role: 'assistant',
    createdAt: new Date(),
    content,
    status,
    metadata: {
      unstable_state: null,
      unstable_annotations: [],
      unstable_data: [],
      steps: [],
      custom: {}
    }
  }
}

export function isPendingAssistantMessageContent(
  content: readonly PendingMessageContentPart[]
): boolean {
  return (
    content.length === 1 &&
    content[0]?.type === 'text' &&
    content[0].text === pendingAssistantMessageText
  )
}
