import { describe, expect, it } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';

const useIPCPath = path.resolve(process.cwd(), 'src/renderer/hooks/useIPC.ts');

describe('useIPC model provider event handling', () => {
  it('stores modelProvider.changed payload in renderer state', () => {
    const source = fs.readFileSync(useIPCPath, 'utf8');
    expect(source).toContain("case 'modelProvider.changed':");
    expect(source).toContain('store.setModelProviderConfig(event.payload as ClientModelProviderConfig);');
    expect(source).toContain("modelProvider.changed received:");
  });
});
