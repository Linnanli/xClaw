import { memo } from 'react';
import { AssistantMessage } from '../message/AssistantMessage';
import type { Message } from '../../../types';

interface AssistantThreadViewProps {
  messages: Message[];
  className?: string;
  emptyPlaceholder?: string;
}

/**
 * Thread-level renderer for assistant UI migration.
 *
 * This is intentionally lightweight and adapter-focused:
 * it consumes canonical message state from the existing store and renders
 * one assistant message row per Message object.
 */
export const AssistantThreadView = memo(function AssistantThreadView({
  messages,
  className,
  emptyPlaceholder,
}: AssistantThreadViewProps): JSX.Element {
  if (messages.length === 0) {
    return <div className={className}>{emptyPlaceholder ?? 'No messages yet'}</div>;
  }

  return (
    <div className={className}>
      {messages.map((message) => (
        <AssistantMessage key={message.id} message={message} />
      ))}
    </div>
  );
}
);
