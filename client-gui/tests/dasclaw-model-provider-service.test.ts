import { describe, expect, it, vi } from 'vitest';
import {
  AdminBackendModelProviderService,
  ModelProviderConfigError,
  clientModelsUrl,
  normalizeAdminClientModels,
} from '../src/main/dasclaw/model-provider-service';

describe('model-provider-service', () => {
  it('builds the admin backend client-models URL with an optional user id', () => {
    expect(clientModelsUrl('http://localhost:3000/')).toBe(
      'http://localhost:3000/api/client-models'
    );
    expect(clientModelsUrl('http://localhost:3000/', 'user-1')).toBe(
      'http://localhost:3000/api/client-models?user_id=user-1'
    );
  });

  it('normalizes admin models into runtime and redacted renderer configs', () => {
    const config = normalizeAdminClientModels([
      adminModel({ model_id: 'gpt-a', is_default: false, api_key: 'secret-a' }),
      adminModel({ model_id: 'gpt-b', is_default: true, api_key: 'secret-b' }),
    ]);

    expect(config.runtime.selectedModel.modelId).toBe('gpt-b');
    expect(config.runtime.models).toEqual([
      expect.objectContaining({
        modelId: 'gpt-a',
        apiBaseUrl: 'https://admin.example/v1',
        apiKey: 'secret-a',
        apiFormat: 'openai',
      }),
      expect.objectContaining({
        modelId: 'gpt-b',
        apiBaseUrl: 'https://admin.example/v1',
        apiKey: 'secret-b',
        apiFormat: 'openai',
      }),
    ]);
    expect(config.renderer.selectedModelId).toBe('gpt-b');
    expect(JSON.stringify(config.renderer)).not.toContain('secret-a');
    expect(JSON.stringify(config.renderer)).not.toContain('secret-b');
    expect(config.renderer.models[1]).toMatchObject({
      modelId: 'gpt-b',
      apiKeyConfigured: true,
      isDefault: true,
    });
  });

  it('rejects missing runtime fields and ambiguous defaults fail-safe', () => {
    expect(() =>
      normalizeAdminClientModels([adminModel({ api_key: null, is_default: true })])
    ).toThrow(ModelProviderConfigError);
    expect(() =>
      normalizeAdminClientModels([
        adminModel({ model_id: 'a', is_default: false }),
        adminModel({ model_id: 'b', is_default: false }),
      ])
    ).toThrow('exactly one default model');
    expect(() =>
      normalizeAdminClientModels([
        adminModel({ model_id: 'same', is_default: true }),
        adminModel({ model_id: 'same', is_default: false }),
      ])
    ).toThrow('duplicate model_id');
  });

  it('fetches /api/client-models and never falls back to local config', async () => {
    const fetchImpl = vi.fn().mockResolvedValue(
      new Response(JSON.stringify([adminModel({ model_id: 'admin-default', is_default: true })]), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      })
    );
    const service = new AdminBackendModelProviderService({
      adminBackendUrl: 'http://admin.test/',
      fetchImpl,
      timeoutMs: 100,
    });

    const config = await service.load();

    expect(fetchImpl).toHaveBeenCalledWith('http://admin.test/api/client-models', {
      method: 'GET',
      signal: expect.any(AbortSignal),
    });
    expect(config.runtime.selectedModel.modelId).toBe('admin-default');
  });

  it('fails safe on admin backend fetch errors', async () => {
    const service = new AdminBackendModelProviderService({
      adminBackendUrl: 'http://admin.test',
      fetchImpl: vi.fn().mockResolvedValue(new Response('nope', { status: 503 })),
      timeoutMs: 100,
    });

    await expect(service.load()).rejects.toThrow(
      'admin backend /api/client-models returned HTTP 503'
    );
  });

  it('wraps invalid admin backend URLs as model provider config errors', async () => {
    const service = new AdminBackendModelProviderService({
      adminBackendUrl: 'not a valid url',
      fetchImpl: vi.fn(),
      timeoutMs: 100,
    });

    await expect(service.load()).rejects.toThrow(ModelProviderConfigError);
  });
});

function adminModel(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    model_id: 'gpt-test',
    display_name: 'GPT Test',
    description: 'admin model',
    provider: 'openai',
    is_default: true,
    capabilities: ['chat'],
    api_base_url: 'https://admin.example/v1',
    api_key: 'secret',
    api_format: 'openai',
    source: 'admin',
    ...overrides,
  };
}
