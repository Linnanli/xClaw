import { ChainOfThoughtPrimitive } from '@assistant-ui/react';
import { ContentBlockView } from '../../message/ContentBlockView';
import type { ContentBlock, Message } from '../../../types';
import type { ContentBlockViewProps, ToolBlockBaseProps } from '../../message/types';

export interface AssistantContentBlockProps extends Omit<ContentBlockViewProps, 'block'> {
  block: ContentBlock;
  isStreaming?: boolean;
  className?: string;
  children?: never;
}

export function AssistantContentBlock({
  block,
  isUser,
  isStreaming,
  allBlocks,
  message,
  className,
}: AssistantContentBlockProps & ToolBlockBaseProps & { message?: Message }): JSX.Element | null {
  const renderedBlock = (
    <ContentBlockView
      block={block}
      isUser={isUser}
      isStreaming={isStreaming}
      allBlocks={allBlocks}
      message={message}
    />
  );

  if (block.type === 'thinking') {
    return (
      <ChainOfThoughtPrimitive.Root
        className={className}
        data-assistant-content-adapter="reasoning"
      >
        {renderedBlock}
      </ChainOfThoughtPrimitive.Root>
    );
  }

  if (block.type === 'tool_use') {
    return (
      <div
        className={className}
        data-assistant-content-adapter="tool-use"
        data-tool-call-id={block.id}
        data-tool-name={block.name}
      >
        {renderedBlock}
      </div>
    );
  }

  if (block.type === 'tool_result') {
    return (
      <div
        className={className}
        data-assistant-content-adapter="tool-result"
        data-tool-call-id={block.toolUseId}
        data-tool-error={block.isError === true ? 'true' : undefined}
      >
        {renderedBlock}
      </div>
    );
  }

  return (
    <div className={className} data-assistant-content-adapter={block.type}>
      {renderedBlock}
    </div>
  );
}
