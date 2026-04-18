import { mockIPC, mockWindows } from '@tauri-apps/api/mocks';

type TauriInvokeHandler = (args: Record<string, unknown>) => unknown;

export type CypressTauriMockConfig = {
  commandHandlers?: Record<string, unknown | TauriInvokeHandler>;
  threadHistoryById?: Record<string, Array<Record<string, unknown>>>;
  threads?: Array<{
    id: string;
    title: string;
    started_at?: string;
    last_activity?: string;
  }>;
};

type MockRuntimeState = {
  createdThreadCount: number;
};

const DEFAULT_MODELS = [
  {
    model_id: 'deepseek-chat',
    display_name: 'DeepSeek Chat',
    description: null,
    provider: 'deepseek',
    provider_display_name: 'DeepSeek',
    is_default: true,
    capabilities: ['chat'],
    source: 'builtin',
  },
];

function buildMockedThreads(config: CypressTauriMockConfig) {
  if (config.threads && config.threads.length > 0) {
    return config.threads;
  }
  return [
    {
      id: 'thread-seeded-1',
      title: 'P2 Seeded Thread',
      started_at: '2025-01-01T00:00:00.000Z',
      last_activity: '2025-01-01T00:00:00.000Z',
    },
  ];
}

function buildDefaultHandlers(
  config: CypressTauriMockConfig,
  state: MockRuntimeState,
): Record<string, unknown | TauriInvokeHandler> {
  const threadHistoryById = { ...(config.threadHistoryById ?? {}) };
  const mockedThreads = buildMockedThreads(config);

  return {
    sync_dlp_rules_from_admin: null,
    subscribe_chat_events: null,
    unsubscribe_chat_events: null,
    scan_user_input: (args) => ({
      had_sensitive_data: false,
      sanitized_content: String(args.content ?? ''),
      was_blocked: false,
      sanitization_stats: {
        total_matches: 0,
        redacted_count: 0,
        blocked_count: 0,
        warned_count: 0,
      },
    }),
    scan_outbound_request: (args) => ({
      had_sensitive_data: false,
      sanitized_content: String(args.body ?? ''),
      was_blocked: false,
      sanitization_stats: {
        total_matches: 0,
        redacted_count: 0,
        blocked_count: 0,
        warned_count: 0,
      },
    }),
    sanitize_for_storage: (args) => String(args.content ?? ''),
    get_available_models: DEFAULT_MODELS,
    get_custom_models: [],
    ic_list_jobs: [],
    ic_list_threads: mockedThreads.map((thread) => ({
      id: thread.id,
      title: thread.title,
      message_count: 3,
      started_at: thread.started_at ?? '2025-01-01T00:00:00.000Z',
      last_activity: thread.last_activity ?? thread.started_at ?? '2025-01-01T00:00:00.000Z',
      channel: 'chat',
    })),
    ic_create_thread: () => {
      state.createdThreadCount += 1;
      return `thread-created-${state.createdThreadCount}`;
    },
    ic_get_thread_history: (args) => {
      const threadId = String(args.threadId ?? '');
      return threadHistoryById[threadId] ?? threadHistoryById['*'] ?? [];
    },
    send_chat_message: (args) => ({
      message_id: `msg-${Date.now()}`,
      success: true,
      thread_id: String(args.threadId ?? 'thread-created-1'),
    }),
  };
}

function resolveInvoke(
  handlers: Record<string, unknown | TauriInvokeHandler>,
  cmd: string,
  args: Record<string, unknown>,
): unknown {
  const handler = handlers[cmd];
  if (typeof handler === 'function') {
    return (handler as TauriInvokeHandler)(args);
  }
  if (handler !== undefined) {
    return handler;
  }
  return null;
}

export function setupCypressTauriMock(): void {
  const win = window as Window & {
    Cypress?: unknown;
    __XCLAW_CYPRESS_TAURI_MOCK__?: CypressTauriMockConfig;
    __XCLAW_CYPRESS_TAURI_EMIT_CHAT_EVENT__?: (payload: Record<string, unknown>) => void;
    __TAURI_INTERNALS__?: {
      invoke?: (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
    };
  };

  if (!win.Cypress) {
    return;
  }

  const config = win.__XCLAW_CYPRESS_TAURI_MOCK__ ?? {};
  const state: MockRuntimeState = { createdThreadCount: 0 };
  const handlers = {
    ...buildDefaultHandlers(config, state),
    ...(config.commandHandlers ?? {}),
  };

  mockWindows('main');
  mockIPC((cmd, args = {}) => resolveInvoke(handlers, cmd, args as Record<string, unknown>), {
    shouldMockEvents: true,
  });

  win.__XCLAW_CYPRESS_TAURI_EMIT_CHAT_EVENT__ = (payload: Record<string, unknown>) => {
    win.__TAURI_INTERNALS__?.invoke?.('plugin:event|emit', {
      event: 'chat-event',
      payload,
    });
  };
}