import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const {
  submitApprovalTicketMock,
  useExternalStoreRuntimeMock,
} = vi.hoisted(() => ({
  submitApprovalTicketMock: vi.fn(),
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
      submitApprovalTicket: submitApprovalTicketMock,
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

import { TauriRuntimeProvider, useApprovalState } from '../TauriRuntimeProvider';

function ApprovalHarness(): ReactNode {
  const { submitForReview } = useApprovalState();

  return (
    <button onClick={() => void submitForReview('req-1', 'shell', 'needs approval')}>
      submit approval
    </button>
  );
}

describe('TauriRuntimeProvider approval runtime', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useExternalStoreRuntimeMock.mockReturnValue({});
  });

  it('重复点击审批提交时只创建一个工单', async () => {
    submitApprovalTicketMock.mockImplementation(
      () => new Promise<string>(() => {})
    );

    render(
      <TauriRuntimeProvider threadId="thread-1">
        <ApprovalHarness />
      </TauriRuntimeProvider>,
    );

    const button = screen.getByRole('button', { name: 'submit approval' });
    fireEvent.click(button);
    fireEvent.click(button);

    await waitFor(() => {
      expect(submitApprovalTicketMock).toHaveBeenCalledTimes(1);
    });
  });
});