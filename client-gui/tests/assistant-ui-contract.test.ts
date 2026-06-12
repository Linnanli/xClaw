import { describe, expect, it } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';

const assistantUiDir = path.resolve(
  process.cwd(),
  'src/renderer/components/assistant-ui',
);

const read = (relativePath: string) =>
  fs.readFileSync(path.join(assistantUiDir, relativePath), 'utf8');

describe('assistant-ui renderer contract', () => {
  it('keeps the thread view as a thin adapter over AssistantMessage', () => {
    const source = read('thread/AssistantThreadView.tsx');

    expect(source).toContain("import { AssistantMessage } from '../message/AssistantMessage';");
    expect(source).toContain('messages.map((message) => (');
    expect(source).toContain('emptyPlaceholder ?? \'No messages yet\'');
  });

  it('delegates content rendering to the existing ContentBlockView implementation', () => {
    const source = read('message/AssistantContentBlock.tsx');

    expect(source).toContain("import { ContentBlockView } from '../../message/ContentBlockView';");
    expect(source).toContain('ContentBlockView');
    expect(source).toContain('block as ContentBlock');
  });

  it('preserves the assistant message behaviors needed for the current baseline', () => {
    const source = read('message/AssistantMessage.tsx');

    expect(source).toContain('navigator.clipboard.writeText(text)');
    expect(source).toContain('mergedResultIds');
    expect(source).toContain("className={`rounded-[1.65rem] px-4 py-3 max-w-[80%] min-w-0 break-words message-user");
    expect(source).toContain('contentBlocks.map((block, index) =>');
  });
});
