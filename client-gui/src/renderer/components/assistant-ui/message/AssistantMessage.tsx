import { useState, memo, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Check, Clock, Copy, XCircle } from 'lucide-react';
import type { Message, ContentBlock, ToolUseContent, ToolResultContent } from '../../../types';
import { AssistantContentBlock } from './AssistantContentBlock';

interface AssistantMessageProps {
  message: Message;
  isStreaming?: boolean;
  className?: string;
}

const resolveContentBlocks = (message: Message): ContentBlock[] => {
  const rawContent = message.content as unknown;
  if (Array.isArray(rawContent)) {
    return rawContent as ContentBlock[];
  }
  return [{ type: 'text', text: String(rawContent ?? '') } as ContentBlock];
};

/**
 * Renders one message node for the assistant-ui thread.
 *
 * Preserves existing semantics:
 * - user bubble styling
 * - assistant messages render blocks directly
 * - copied text from text blocks
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
  const contentBlocks = resolveContentBlocks(message);
  const [copied, setCopied] = useState(false);

  const mergedResultIds = useMemo(() => {
    const ids = new Set<string>();
    for (const block of contentBlocks) {
      if (block.type === 'tool_use') {
        const tu = block as ToolUseContent;
        const matchingResult = contentBlocks.find(
          (candidate) =>
            candidate.type === 'tool_result' &&
            (candidate as ToolResultContent).toolUseId === tu.id,
        );
        if (matchingResult) {
          ids.add((matchingResult as ToolResultContent).toolUseId);
        }
      }
    }
    return ids;
  }, [contentBlocks]);

  const getTextContent = (): string =>
    contentBlocks
      .filter((block) => block.type === 'text')
      .map((block) => (block as { type: 'text'; text: string }).text)
      .join('\n');

  const handleCopy = async () => {
    const text = getTextContent();
    if (!text) {
      return;
    }

    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // Clipboard unavailable in some embedded environments.
    }
  };

  return (
    <div className={className}>
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

          <button
            onClick={handleCopy}
            className="mt-1 w-6 h-6 flex items-center justify-center rounded-md bg-surface-muted hover:bg-surface-active transition-all opacity-0 group-hover:opacity-100 flex-shrink-0"
            title={t('messageCard.copyMessage')}
            type="button"
          >
            {copied ? (
              <Check className="w-3 h-3 text-success" />
            ) : (
              <Copy className="w-3 h-3 text-text-muted" />
            )}
          </button>
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
    </div>
  );
});
