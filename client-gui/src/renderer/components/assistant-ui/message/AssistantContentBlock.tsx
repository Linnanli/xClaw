import { ContentBlockView } from '../../message/ContentBlockView';
import type { ContentBlock, Message } from '../../../types';
import type { ContentBlockViewProps, ToolBlockBaseProps } from '../../message/types';

export interface AssistantContentBlockProps extends Omit<ContentBlockViewProps, 'block'> {
  block: ContentBlock;
  isStreaming?: boolean;
  className?: string;
  children?: never;
}

/**
 * AssistantUI-compatible content block renderer.
 *
 * This keeps the existing ContentBlock rendering behavior intact (text/image/file/tool/use/result/thinking)
 * while exposing a small adapter surface used by the new message/thread layer.
 */
export const AssistantContentBlock = ({
  block,
  isUser,
  isStreaming,
  allBlocks,
  message,
  className,
}: AssistantContentBlockProps & ToolBlockBaseProps & { message?: Message }): JSX.Element | null => {
  return (
    <div className={className}>
      <ContentBlockView
        block={block as ContentBlock}
        isUser={isUser}
        isStreaming={isStreaming}
        allBlocks={allBlocks}
        message={message}
      />
    </div>
  );
};
