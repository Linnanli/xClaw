import { memo, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { ActionBarPrimitive, MessagePrimitive } from '@assistant-ui/react';
import { Check, Clock, Copy, XCircle } from 'lucide-react';
import type { Message, ToolResultContent } from '../../../types';
import { getMergedToolResultIds, resolveAssistantContentBlocks } from '../assistantAdapters';
import { AssistantContentBlock } from './AssistantContentBlock';

interface AssistantMessageProps {
  message: Message;
  isStreaming?: boolean;
  className?: string;
}

/**
 * Renders one message node for the assistant-ui thread.
 *
 * Preserves existing semantics:
 * - user bubble styling
 * - assistant messages render blocks directly
 * - copy action text from assistant-ui converted text parts
 * - tool_result blocks are merged with matching tool_use blocks
 */
export const AssistantMessage = memo(function AssistantMessage({
  message,
  isStreaming,
  className,
}: AssistantMessageProps) {
  const { t } = useTranslation();
  const isUser = message.role === 'user';
  const isQueued = message.localStatus === 'queued';
  const isCancelled = message.localStatus === 'cancelled';
  const contentBlocks = resolveAssistantContentBlocks(message);
  const mergedResultIds = useMemo(() => getMergedToolResultIds(contentBlocks), [contentBlocks]);

  return (
    <MessagePrimitive.Root
      className={className}
      data-client-message-id={message.id}
      data-client-message-role={message.role}
    >
      {isUser ? (
        <div className="flex items-start gap-2 justify-end group">
          <div
            className={`rounded-[1.65rem] px-4 py-3 max-w-[80%] min-w-0 break-words message-user ${
              isQueued ? 'opacity-70 border-dashed' : ''
            } ${isCancelled ? 'opacity-60' : ''}`}
          >
            {isQueued && (
              <div className="mb-1 flex items-center gap-1 text-[11px] text-text-muted">
                <Clock className="w-3 h-3" />
                <span>{t('messageCard.queued')}</span>
              </div>
            )}
            {isCancelled && (
              <div className="mb-1 flex items-center gap-1 text-[11px] text-text-muted">
                <XCircle className="w-3 h-3" />
                <span>{t('messageCard.cancelled')}</span>
              </div>
            )}
            {contentBlocks.length === 0 ? (
              <span className="text-text-muted italic">{t('messageCard.emptyMessage')}</span>
            ) : (
              contentBlocks.map((block, index) => (
                <AssistantContentBlock
                  key={
                    'id' in block && typeof (block as { id?: string }).id === 'string'
                      ? (block as { id: string }).id
                      : `assistant-block-${index}`
                  }
                  block={block}
                  isUser={isUser}
                  isStreaming={isStreaming}
                  allBlocks={contentBlocks}
                  message={message}
                />
              ))
            )}
          </div>

          <ActionBarPrimitive.Root>
            <ActionBarPrimitive.Copy asChild copiedDuration={2000}>
              <button
                className="group/copy mt-1 w-6 h-6 flex items-center justify-center rounded-md bg-surface-muted hover:bg-surface-active transition-all opacity-0 group-hover:opacity-100 flex-shrink-0"
                title={t('messageCard.copyMessage')}
                type="button"
              >
                <Check className="w-3 h-3 text-success hidden group-data-[copied=true]/copy:block" />
                <Copy className="w-3 h-3 text-text-muted block group-data-[copied=true]/copy:hidden" />
              </button>
            </ActionBarPrimitive.Copy>
          </ActionBarPrimitive.Root>
        </div>
      ) : (
        <div className="space-y-1.5">
          {contentBlocks.length === 0 ? (
            <span className="text-text-muted italic">{t('messageCard.emptyText')}</span>
          ) : (
            contentBlocks.map((block, index) => {
              if (
                block.type === 'tool_result' &&
                mergedResultIds.has((block as ToolResultContent).toolUseId)
              ) {
                return null;
              }

              return (
                <AssistantContentBlock
                  key={
                    'id' in block && typeof (block as { id?: string }).id === 'string'
                      ? (block as { id: string }).id
                      : `assistant-block-${index}`
                  }
                  block={block}
                  isUser={isUser}
                  isStreaming={isStreaming}
                  allBlocks={contentBlocks}
                  message={message}
                />
              );
            })
          )}
        </div>
      )}
    </MessagePrimitive.Root>
  );
});
