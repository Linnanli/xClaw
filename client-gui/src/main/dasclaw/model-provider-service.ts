export interface AdminClientModelConfig {
  model_id: string;
  display_name: string;
  description?: string | null;
  provider: string;
  is_default: boolean;
  capabilities?: unknown;
  api_base_url?: string | null;
  api_key?: string | null;
  api_format?: string | null;
  source?: string | null;
}

export interface AppServerClientModelConfig {
  modelId: string;
  displayName?: string;
  provider: string;
  apiBaseUrl: string;
  apiKey: string;
  apiFormat: string;
  source?: string;
  capabilities: string[];
}

export interface AppServerModelProviderConfig {
  models: AppServerClientModelConfig[];
  selectedModel: AppServerClientModelConfig;
}

export interface RendererClientModelConfig {
  modelId: string;
  displayName: string;
  description?: string;
  provider: string;
  isDefault: boolean;
  capabilities: string[];
  apiBaseUrl: string;
  apiFormat: string;
  source: string;
  apiKeyConfigured: boolean;
}

export interface RendererModelProviderConfig {
  models: RendererClientModelConfig[];
  selectedModelId: string;
}

export interface ResolvedModelProviderConfig {
  runtime: AppServerModelProviderConfig;
  renderer: RendererModelProviderConfig;
}

export interface ModelProviderService {
  load(): Promise<ResolvedModelProviderConfig>;
}

interface AdminBackendModelProviderServiceOptions {
  adminBackendUrl?: string;
  userId?: string;
  fetchImpl?: typeof fetch;
  timeoutMs?: number;
}

interface NormalizedClientModel {
  runtime: AppServerClientModelConfig;
  renderer: RendererClientModelConfig;
  isDefault: boolean;
}

const DEFAULT_ADMIN_BACKEND_URL = 'http://localhost:3000';
const DEFAULT_FETCH_TIMEOUT_MS = 5000;

export class ModelProviderConfigError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'ModelProviderConfigError';
  }
}

export class AdminBackendModelProviderService implements ModelProviderService {
  private readonly adminBackendUrl: string;
  private readonly userId?: string;
  private readonly fetchImpl: typeof fetch;
  private readonly timeoutMs: number;

  constructor(options: AdminBackendModelProviderServiceOptions = {}) {
    this.adminBackendUrl =
      options.adminBackendUrl?.trim() ||
      process.env.ADMIN_BACKEND_URL?.trim() ||
      DEFAULT_ADMIN_BACKEND_URL;
    this.userId = options.userId?.trim() || undefined;
    this.fetchImpl = options.fetchImpl ?? fetch;
    this.timeoutMs = options.timeoutMs ?? DEFAULT_FETCH_TIMEOUT_MS;
  }

  async load(): Promise<ResolvedModelProviderConfig> {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), this.timeoutMs);

    try {
      const url = clientModelsUrl(this.adminBackendUrl, this.userId);
      const response = await this.fetchImpl(url, {
        method: 'GET',
        signal: controller.signal,
      });
      if (!response.ok) {
        throw new ModelProviderConfigError(
          `admin backend /api/client-models returned HTTP ${response.status}`
        );
      }

      const body = await response.json();
      return normalizeAdminClientModels(body);
    } catch (error) {
      if (error instanceof ModelProviderConfigError) {
        throw error;
      }
      throw new ModelProviderConfigError(
        `failed to fetch admin backend /api/client-models: ${errorMessage(error)}`
      );
    } finally {
      clearTimeout(timeout);
    }
  }
}

export function clientModelsUrl(adminBackendUrl: string, userId?: string): string {
  const base = adminBackendUrl.trim().replace(/\/+$/, '') || DEFAULT_ADMIN_BACKEND_URL;
  const url = new URL(`${base}/api/client-models`);
  if (userId?.trim()) {
    url.searchParams.set('user_id', userId.trim());
  }
  return url.toString();
}

export function normalizeAdminClientModels(body: unknown): ResolvedModelProviderConfig {
  if (!Array.isArray(body) || body.length === 0) {
    throw new ModelProviderConfigError('admin backend /api/client-models returned no models');
  }

  const seenModelIds = new Set<string>();
  const models = body.map((value, index) => normalizeAdminClientModel(value, index));
  for (const model of models) {
    if (seenModelIds.has(model.runtime.modelId)) {
      throw new ModelProviderConfigError(
        `admin backend /api/client-models returned duplicate model_id: ${model.runtime.modelId}`
      );
    }
    seenModelIds.add(model.runtime.modelId);
  }

  const defaultModels = models.filter((model) => model.isDefault);
  if (defaultModels.length !== 1) {
    throw new ModelProviderConfigError(
      'admin backend /api/client-models must return exactly one default model'
    );
  }

  return {
    runtime: {
      models: models.map((model) => model.runtime),
      selectedModel: defaultModels[0].runtime,
    },
    renderer: {
      models: models.map((model) => model.renderer),
      selectedModelId: defaultModels[0].runtime.modelId,
    },
  };
}

function normalizeAdminClientModel(value: unknown, index: number): NormalizedClientModel {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new ModelProviderConfigError(`model[${index}] must be an object`);
  }

  const raw = value as Partial<AdminClientModelConfig>;
  const modelId = requiredString(raw.model_id, `model[${index}].model_id`);
  const displayName = requiredString(raw.display_name, `model[${index}].display_name`);
  const provider = requiredString(raw.provider, `model[${index}].provider`);
  const apiBaseUrl = requiredString(raw.api_base_url, `model[${index}].api_base_url`);
  const apiKey = requiredString(raw.api_key, `model[${index}].api_key`);
  const apiFormat = optionalString(raw.api_format)?.trim() || 'openai';
  const source = optionalString(raw.source)?.trim() || 'admin';
  const capabilities = normalizeCapabilities(raw.capabilities);
  const isDefault = raw.is_default === true;
  const description = optionalString(raw.description)?.trim();

  return {
    runtime: {
      modelId,
      displayName,
      provider,
      apiBaseUrl,
      apiKey,
      apiFormat,
      source,
      capabilities,
    },
    renderer: {
      modelId,
      displayName,
      description: description || undefined,
      provider,
      isDefault,
      capabilities,
      apiBaseUrl,
      apiFormat,
      source,
      apiKeyConfigured: true,
    },
    isDefault,
  };
}

function requiredString(value: unknown, field: string): string {
  if (typeof value !== 'string' || value.trim() === '') {
    throw new ModelProviderConfigError(`${field} is required`);
  }
  return value.trim();
}

function optionalString(value: unknown): string | undefined {
  return typeof value === 'string' ? value : undefined;
}

function normalizeCapabilities(value: unknown): string[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value.filter((item): item is string => typeof item === 'string');
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
