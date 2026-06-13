import type { ContentBlock, Message, ToolUseContent } from '../../types';

export function resolveAssistantContentBlocks(message: Message): ContentBlock[] {
  const rawContent = message.content as unknown;
  if (Array.isArray(rawContent)) {
    return rawContent as ContentBlock[];
  }
  return [{ type: 'text', text: String(rawContent ?? '') }];
}

export function getMergedToolResultIds(contentBlocks: ContentBlock[]): ReadonlySet<string> {
  const toolUseIds = new Set(
    contentBlocks
      .filter((block): block is ToolUseContent => block.type === 'tool_use')
      .map((block) => block.id)
  );
  const mergedResultIds = new Set<string>();

  for (const block of contentBlocks) {
    if (block.type === 'tool_result' && toolUseIds.has(block.toolUseId)) {
      mergedResultIds.add(block.toolUseId);
    }
  }

  return mergedResultIds;
}
