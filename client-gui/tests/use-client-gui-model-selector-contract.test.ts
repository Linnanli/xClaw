import { describe, expect, it } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';

const hookPath = path.resolve(process.cwd(), 'src/renderer/hooks/useClientGuiModelSelector.ts');
const ipcPath = path.resolve(process.cwd(), 'src/renderer/hooks/useIPC.ts');

describe('useClientGuiModelSelector contract', () => {
  it('keeps the renderer bridge for modelProvider.changed and selector persistence', () => {
    const hookSource = fs.readFileSync(hookPath, 'utf8');
    const ipcSource = fs.readFileSync(ipcPath, 'utf8');

    expect(ipcSource).toContain("case 'modelProvider.changed':");
    expect(ipcSource).toContain('store.setModelProviderConfig(event.payload as ClientModelProviderConfig);');
    expect(hookSource).toContain('modelProviderConfig from renderer store, updated by modelProvider.changed');
    expect(hookSource).toContain('setModelProviderConfig(serverConfig)');
    expect(hookSource).toContain('setModelProviderConfig(config)');
    expect(hookSource).toContain("window.electronAPI.config.listModels({");
    expect(hookSource).toContain("window.electronAPI.config.save({ model: modelId })");
  });
});
