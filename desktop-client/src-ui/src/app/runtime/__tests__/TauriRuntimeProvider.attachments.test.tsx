import { render, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { useExternalStoreRuntimeMock } = vi.hoisted(() => ({
  useExternalStoreRuntimeMock: vi.fn(() => ({})),
}));

vi.mock('@assistant-ui/react', () => ({
  AssistantRuntimeProvider: ({ children }: { children: ReactNode }) => children,
  useExternalStoreRuntime: useExternalStoreRuntimeMock,
}));

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn().mockResolvedValue(undefined) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));

vi.mock('../../hooks/useDlpScan', () => ({
  useDlpScan: () => ({
    scanUserInput: vi.fn().mockResolvedValue({
      had_sensitive_data: false,
      sanitized_content: '',
      was_blocked: false,
      block_reason: null,
      sanitization_stats: { total_matches: 0, redacted_count: 0, blocked_count: 0, warned_count: 0 },
    }),
  }),
}));

vi.mock('../../utils/tracing', () => ({
  tracing: {
    info: vi.fn(),
    debug: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  },
}));

vi.mock('../../utils/tauri', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../utils/tauri')>();
  return {
    ...actual,
    approvalApi: {
      approve: vi.fn().mockResolvedValue(undefined),
      deny: vi.fn().mockResolvedValue(undefined),
      listPending: vi.fn().mockResolvedValue([]),
    },
    modelApi: {
      getAvailableModels: vi.fn().mockResolvedValue([]),
    },
    threadApi: {
      getMessages: vi.fn().mockResolvedValue([]),
      createThread: vi.fn().mockResolvedValue({ id: 'thread-1' }),
      interruptThread: vi.fn().mockResolvedValue(undefined),
      finalizeThread: vi.fn().mockResolvedValue(undefined),
    },
  };
});

import { TauriRuntimeProvider } from '../TauriRuntimeProvider';

describe('TauriRuntimeProvider attachments runtime', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useExternalStoreRuntimeMock.mockReturnValue({});
  });

  it('为 composer 配置 attachments adapter', async () => {
    render(
      <TauriRuntimeProvider threadId={null}>
        <div>child</div>
      </TauriRuntimeProvider>,
    );

    await waitFor(() => {
      expect(useExternalStoreRuntimeMock).toHaveBeenCalled();
    });

    const options = useExternalStoreRuntimeMock.mock.calls[0]?.[0];
    expect(options?.adapters?.attachments).toBeTruthy();
    expect(options?.adapters?.attachments.accept).toContain('image/*');
    expect(typeof options?.adapters?.attachments.add).toBe('function');
    expect(typeof options?.adapters?.attachments.send).toBe('function');
  });
});