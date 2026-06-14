import type {
  AppendMessage,
  ThreadAssistantMessage,
  ThreadMessage,
  ThreadUserMessage
} from '@assistant-ui/react'

export type AssistantModelOption = {
  id: string
  name: string
}

export const defaultAssistantModelId = 'dasclaw-default'

export const assistantModelOptions: AssistantModelOption[] = [
  { id: defaultAssistantModelId, name: 'Dasclaw Default' },
  { id: 'dasclaw-fast', name: 'Dasclaw Fast' },
  { id: 'dasclaw-deep', name: 'Dasclaw Deep' }
]

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
  return {
    id,
    role: 'assistant',
    createdAt: new Date(),
    content: [{ type: 'text', text }],
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
