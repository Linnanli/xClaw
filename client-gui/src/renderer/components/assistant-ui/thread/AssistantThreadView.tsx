import { memo } from 'react';
import { AuiIf, ThreadPrimitive } from '@assistant-ui/react';
import { AssistantMessage } from '../message/AssistantMessage';
import type { Message } from '../../../types';
import { useClientMessageByAssistantId } from '../runtime/AssistantRuntimeAdapter';

interface AssistantThreadViewProps {
  messages: Message[];
  className?: string;
  emptyPlaceholder?: string;
}

function AssistantThreadMessage({ messageId }: { messageId: string }): JSX.Element | null {
  const message = useClientMessageByAssistantId(messageId);
  if (!message) {
    return null;
  }

  return <AssistantMessage message={message} isStreaming={message.streaming === true} />;
}

export const AssistantThreadView = memo(function AssistantThreadView({
  messages,
  className,
  emptyPlaceholder,
}: AssistantThreadViewProps): JSX.Element {
  return (
    <ThreadPrimitive.Root data-message-count={messages.length}>
      <ThreadPrimitive.Viewport className={className} autoScroll={false}>
        <AuiIf condition={(state) => state.thread.isEmpty}>
          <div>{emptyPlaceholder ?? 'No messages yet'}</div>
        </AuiIf>

        <ThreadPrimitive.Messages>
          {({ message }) => <AssistantThreadMessage messageId={message.id} />}
        </ThreadPrimitive.Messages>
      </ThreadPrimitive.Viewport>
    </ThreadPrimitive.Root>
  );
});
