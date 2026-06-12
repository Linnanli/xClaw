import { describe, expect, it } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';

const chatViewPath = path.resolve(process.cwd(), 'src/renderer/components/ChatView.tsx');

describe('ChatView model selector integration', () => {
  it('renders the assistant-ui model selector instead of the static model label', () => {
    const source = fs.readFileSync(chatViewPath, 'utf8');
    expect(source).toContain("AssistantModelSelector");
    expect(source).not.toContain("appConfig?.model || t('chat.noModel')");
  });
});
