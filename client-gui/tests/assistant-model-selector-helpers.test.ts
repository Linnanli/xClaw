import { describe, expect, it } from 'vitest';
import {
  normalizeConfigModelOptions,
  normalizeServerModelOptions,
  resolveModelDisplayName,
} from '../src/renderer/components/assistant-ui/model-selector-utils';
import type { ClientModelProviderConfig } from '../src/renderer/types';

describe('AssistantModelSelector helpers', () => {
  it('normalizes server-side model provider config into dropdown options', () => {
    const config: ClientModelProviderConfig = {
      selectedModelId: 'server-gpt',
      models: [
        {
          modelId: 'server-gpt',
          displayName: 'Server GPT',
          provider: 'openai',
          isDefault: true,
          capabilities: ['chat'],
          apiBaseUrl: 'https://example.test/v1',
          apiFormat: 'openai',
          source: 'admin',
          apiKeyConfigured: true,
        },
        {
          modelId: 'server-next',
          displayName: 'Server Next',
          provider: 'openai',
          isDefault: false,
          capabilities: ['chat'],
          apiBaseUrl: 'https://example.test/v1',
          apiFormat: 'openai',
          source: 'admin',
          apiKeyConfigured: true,
        },
      ],
    };

    expect(normalizeServerModelOptions(config)).toEqual([
      { modelId: 'server-gpt', displayName: 'Server GPT', isDefault: true },
      { modelId: 'server-next', displayName: 'Server Next', isDefault: false },
    ]);
  });

  it('normalizes config-backed model info into dropdown options', () => {
    expect(
      normalizeConfigModelOptions([
        { id: 'o4-mini', name: 'o4-mini' },
        { id: 'claude-sonnet', name: 'Claude Sonnet' },
      ])
    ).toEqual([
      { modelId: 'o4-mini', displayName: 'o4-mini', isDefault: true },
      { modelId: 'claude-sonnet', displayName: 'Claude Sonnet', isDefault: false },
    ]);
  });

  it('resolves the visible label from committed state first and falls back safely', () => {
    const options = [
      { modelId: 'o4-mini', displayName: 'o4-mini', isDefault: true },
      { modelId: 'claude-sonnet', displayName: 'Claude Sonnet', isDefault: false },
    ];

    expect(resolveModelDisplayName('claude-sonnet', options, 'No model')).toBe('Claude Sonnet');
    expect(resolveModelDisplayName('missing', options, 'No model')).toBe('missing');
    expect(resolveModelDisplayName('', [], 'No model')).toBe('No model');
  });
});
