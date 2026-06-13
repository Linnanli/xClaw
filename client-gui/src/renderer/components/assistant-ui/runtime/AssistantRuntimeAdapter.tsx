import { createContext, useCallback, useContext, useMemo, type ReactNode } from 'react';
import {
  AssistantRuntimeProvider,
  useExternalStoreRuntime,
  type AppendMessage,
  type ThreadMessageLike,
} from '@assistant-ui/react';
import type {
  ContentBlock,
  Message,
  MessageRole,
  ToolResultContent,
  ToolUseContent,
} from '../../../types';

interface AssistantRuntimeAdapterProps {
  messages: Message[];
  isRunning: boolean;
  isSendDisabled: boolean;
  onNew: (message: AppendMessage) => Promise<void> | void;
  onCancel: () => Promise<void> | void;
  children: ReactNode;
}

const ClientMessageByIdContext = createContext<ReadonlyMap<string, Message>>(new Map());

export function useClientMessageByAssistantId(messageId: string): Message | undefined {
  return useContext(ClientMessageByIdContext).get(messageId);
}

function toAssistantRole(role: MessageRole): ThreadMessageLike['role'] {
  if (role === 'system') {
    return 'system';
  }
  return role;
}

type RuntimeContentPart = Extract<ThreadMessageLike['content'], readonly unknown[]>[number];

function findMatchingToolResult(
  block: ToolUseContent,
  allBlocks: ContentBlock[]
): ToolResultContent | undefined {
  return allBlocks.find(
    (candidate): candidate is ToolResultContent =>
      candidate.type === 'tool_result' && candidate.toolUseId === block.id
  );
}

function hasMatchingToolUse(block: ToolResultContent, allBlocks: ContentBlock[]): boolean {
  return allBlocks.some(
    (candidate): candidate is ToolUseContent =>
      candidate.type === 'tool_use' && candidate.id === block.toolUseId
  );
}

function contentBlockToRuntimePart(
  block: ContentBlock,
  allBlocks: ContentBlock[]
): RuntimeContentPart | null {
  switch (block.type) {
    case 'text':
      return { type: 'text', text: block.text };
    case 'thinking':
      return { type: 'reasoning', text: block.thinking };
    case 'image':
      return {
        type: 'image',
        image: `data:${block.source.media_type};base64,${block.source.data}`,
      };
    case 'file_attachment':
      return {
        type: 'file',
        data: block.inlineDataBase64 ?? '',
        mimeType: block.mimeType ?? 'application/octet-stream',
        filename: block.filename,
      };
    case 'tool_use': {
      const matchingResult = findMatchingToolResult(block, allBlocks);
      return {
        type: 'tool-call',
        toolCallId: block.id,
        toolName: block.displayName ?? block.name,
        argsText: JSON.stringify(block.input, null, 2),
        result: matchingResult?.content,
        isError: matchingResult?.isError,
      };
    }
    case 'tool_result':
      if (hasMatchingToolUse(block, allBlocks)) {
        return null;
      }
      return {
        type: 'text',
        text: block.content,
      };
  }
}

export function convertClientMessageToThreadMessage(
  message: Message,
  index: number
): ThreadMessageLike {
  const content = message.content
    .map((block) => contentBlockToRuntimePart(block, message.content))
    .filter((part): part is RuntimeContentPart => part !== null);

  return {
    id: message.id,
    role: toAssistantRole(message.role),
    content: content.length > 0 ? content : '',
    createdAt: new Date(message.timestamp),
    metadata: {
      custom: {
        clientGuiIndex: index,
        sessionId: message.sessionId,
        localStatus: message.localStatus,
        streaming: message.streaming === true,
      },
    },
  };
}

export function AssistantRuntimeAdapter({
  messages,
  isRunning,
  isSendDisabled,
  onNew,
  onCancel,
  children,
}: AssistantRuntimeAdapterProps): JSX.Element {
  const messageById = useMemo(() => {
    return new Map(messages.map((message) => [message.id, message]));
  }, [messages]);

  const handleNew = useCallback(
    async (message: AppendMessage) => {
      await onNew(message);
    },
    [onNew]
  );

  const handleCancel = useCallback(async () => {
    await onCancel();
  }, [onCancel]);

  // client-gui shows first-token waiting state outside the message list. Keeping
  // assistant-ui running only after assistant content exists avoids an unmapped
  // optimistic assistant message in ThreadPrimitive.Messages.
  const runtimeIsRunning = isRunning && messages.at(-1)?.role === 'assistant';

  const runtime = useExternalStoreRuntime<Message>({
    messages,
    isRunning: runtimeIsRunning,
    isDisabled: isSendDisabled,
    onNew: handleNew,
    onCancel: handleCancel,
    convertMessage: convertClientMessageToThreadMessage,
    unstable_capabilities: {
      copy: true,
    },
  });

  return (
    <AssistantRuntimeProvider runtime={runtime}>
      <ClientMessageByIdContext.Provider value={messageById}>
        {children}
      </ClientMessageByIdContext.Provider>
    </AssistantRuntimeProvider>
  );
}
