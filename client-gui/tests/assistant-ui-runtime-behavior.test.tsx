// @vitest-environment happy-dom
import { createRef, type ReactElement } from 'react';
import { act } from 'react-dom/test-utils';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import { ThreadPrimitive } from '@assistant-ui/react';
import { AssistantRuntimeAdapter } from '../src/renderer/components/assistant-ui/runtime/AssistantRuntimeAdapter';
import { AssistantThreadView } from '../src/renderer/components/assistant-ui/thread/AssistantThreadView';
import { AssistantComposer } from '../src/renderer/components/assistant-ui/composer/AssistantComposer';
import { AssistantSuggestionList } from '../src/renderer/components/assistant-ui/suggestion/AssistantSuggestionList';
import type { Message } from '../src/renderer/types';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

vi.mock('../src/renderer/components/message/ContentBlockView', () => ({
  ContentBlockView: ({
    block,
  }: {
    block: {
      type: string;
      text?: string;
      thinking?: string;
      content?: string;
    };
  }) => (
    <span data-content-block-type={block.type}>
      {block.text ?? block.thinking ?? block.content ?? ''}
    </span>
  ),
}));

const containers: Array<{ container: HTMLDivElement; root: Root }> = [];

function render(element: ReactElement): HTMLDivElement {
  const container = document.createElement('div');
  document.body.appendChild(container);
  const root = createRoot(container);

  act(() => {
    root.render(element);
  });

  containers.push({ container, root });
  return container;
}

function makeMessage(overrides: Partial<Message>): Message {
  return {
    id: overrides.id ?? 'message-1',
    sessionId: overrides.sessionId ?? 'session-1',
    role: overrides.role ?? 'user',
    content: overrides.content ?? [{ type: 'text', text: 'hello' }],
    timestamp: overrides.timestamp ?? 1,
    streaming: overrides.streaming,
    api: overrides.api,
    provider: overrides.provider,
    model: overrides.model,
    tokenUsage: overrides.tokenUsage,
    localStatus: overrides.localStatus,
    executionTimeMs: overrides.executionTimeMs,
  };
}

function RuntimeMessageProbe(): JSX.Element {
  return (
    <ThreadPrimitive.Root>
      <ThreadPrimitive.Messages>
        {({ message }) => (
          <span data-runtime-message-id={message.id} data-runtime-message-role={message.role} />
        )}
      </ThreadPrimitive.Messages>
    </ThreadPrimitive.Root>
  );
}

beforeAll(() => {
  (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
});

afterEach(() => {
  for (const { container, root } of containers.splice(0)) {
    act(() => {
      root.unmount();
    });
    container.remove();
  }
  vi.restoreAllMocks();
});

describe('assistant-ui runtime behavior', () => {
  it('does not expose an unmapped optimistic assistant message before first assistant content', () => {
    const userMessage = makeMessage({
      id: 'user-1',
      role: 'user',
      content: [{ type: 'text', text: 'waiting' }],
    });

    const container = render(
      <AssistantRuntimeAdapter
        messages={[userMessage]}
        isRunning={true}
        isSendDisabled={false}
        onNew={vi.fn()}
        onCancel={vi.fn()}
      >
        <RuntimeMessageProbe />
      </AssistantRuntimeAdapter>
    );

    expect(container.querySelectorAll('[data-runtime-message-role="user"]')).toHaveLength(1);
    expect(container.querySelectorAll('[data-runtime-message-role="assistant"]')).toHaveLength(0);
  });

  it('uses the assistant-ui copy primitive as the only clipboard writer', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText },
    });
    const userMessage = makeMessage({
      id: 'user-copy',
      role: 'user',
      content: [{ type: 'text', text: 'copy me' }],
    });

    const container = render(
      <AssistantRuntimeAdapter
        messages={[userMessage]}
        isRunning={false}
        isSendDisabled={false}
        onNew={vi.fn()}
        onCancel={vi.fn()}
      >
        <AssistantThreadView messages={[userMessage]} />
      </AssistantRuntimeAdapter>
    );

    const copyButton = container.querySelector<HTMLButtonElement>(
      'button[title="messageCard.copyMessage"]'
    );

    expect(copyButton).not.toBeNull();

    await act(async () => {
      copyButton?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(writeText).toHaveBeenCalledTimes(1);
    expect(writeText).toHaveBeenCalledWith('copy me');
  });

  it('delegates controlled composer submits once without calling runtime onNew', async () => {
    const onSubmit = vi.fn();
    const onRuntimeNew = vi.fn();
    const textareaRef = createRef<HTMLTextAreaElement>();

    const container = render(
      <AssistantRuntimeAdapter
        messages={[]}
        isRunning={false}
        isSendDisabled={false}
        onNew={onRuntimeNew}
        onCancel={vi.fn()}
      >
        <AssistantComposer
          prompt="hello"
          onPromptChange={vi.fn()}
          textareaRef={textareaRef}
          isSubmitting={false}
          pastedImages={[]}
          attachedFiles={[]}
          onRemoveImage={vi.fn()}
          onRemoveFile={vi.fn()}
          onPaste={vi.fn()}
          onSubmit={onSubmit}
          placeholder="Type"
          attachmentImageAlt={(index) => `image-${index}`}
          rightActions={<button type="submit">Send</button>}
        />
      </AssistantRuntimeAdapter>
    );

    const form = container.querySelector('form');
    expect(form).not.toBeNull();

    await act(async () => {
      form?.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
      await Promise.resolve();
    });

    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(onRuntimeNew).not.toHaveBeenCalled();
  });

  it('renders controlled draft attachments through the assistant attachment adapter', async () => {
    const onRemoveImage = vi.fn();
    const onRemoveFile = vi.fn();
    const textareaRef = createRef<HTMLTextAreaElement>();

    const container = render(
      <AssistantRuntimeAdapter
        messages={[]}
        isRunning={false}
        isSendDisabled={false}
        onNew={vi.fn()}
        onCancel={vi.fn()}
      >
        <AssistantComposer
          prompt=""
          onPromptChange={vi.fn()}
          textareaRef={textareaRef}
          isSubmitting={false}
          pastedImages={[{ url: 'blob:image-one', base64: 'AAA', mediaType: 'image/png' }]}
          attachedFiles={[
            {
              name: 'notes.txt',
              path: '/tmp/notes.txt',
              size: 12,
              type: 'text/plain',
            },
          ]}
          onRemoveImage={onRemoveImage}
          onRemoveFile={onRemoveFile}
          onPaste={vi.fn()}
          onSubmit={vi.fn()}
          placeholder="Type"
          attachmentImageAlt={(index) => `image-${index + 1}`}
        />
      </AssistantRuntimeAdapter>
    );

    expect(container.querySelector('[data-assistant-attachment-kind="image"]')).not.toBeNull();
    expect(container.querySelector('[data-assistant-attachment-kind="file"]')).not.toBeNull();
    expect(container.textContent).toContain('notes.txt');

    await act(async () => {
      container
        .querySelector<HTMLButtonElement>('button[aria-label="Remove pasted image 1"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      container
        .querySelector<HTMLButtonElement>('button[aria-label="Remove attached file notes.txt"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(onRemoveImage).toHaveBeenCalledWith(0);
    expect(onRemoveFile).toHaveBeenCalledWith(0);
  });

  it('fills welcome suggestions through the assistant-ui suggestion adapter', async () => {
    const onSelect = vi.fn();

    const container = render(
      <AssistantRuntimeAdapter
        messages={[]}
        isRunning={false}
        isSendDisabled={false}
        onNew={vi.fn()}
        onCancel={vi.fn()}
      >
        <AssistantSuggestionList
          selectedId={null}
          onSelect={onSelect}
          suggestions={[
            {
              id: 'create',
              label: 'Create file',
              prompt: 'Create a file',
              icon: ({ className }) => <span className={className} data-testid="icon" />,
            },
          ]}
        />
      </AssistantRuntimeAdapter>
    );

    await act(async () => {
      container
        .querySelector<HTMLButtonElement>('[data-assistant-suggestion-id="create"]')
        ?.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));
      await Promise.resolve();
    });

    expect(onSelect).toHaveBeenCalledWith('create', 'Create a file');
  });

  it('marks thinking and tool blocks with assistant-ui content adapter metadata', () => {
    const assistantMessage = makeMessage({
      id: 'assistant-rich',
      role: 'assistant',
      content: [
        { type: 'thinking', thinking: 'reasoning' },
        { type: 'tool_use', id: 'tool-1', name: 'mcp__server__tool', input: {} },
        { type: 'tool_result', toolUseId: 'tool-1', content: 'done' },
        { type: 'tool_result', toolUseId: 'orphan-tool', content: 'failed', isError: true },
      ],
    });

    const container = render(
      <AssistantRuntimeAdapter
        messages={[assistantMessage]}
        isRunning={false}
        isSendDisabled={false}
        onNew={vi.fn()}
        onCancel={vi.fn()}
      >
        <AssistantThreadView messages={[assistantMessage]} />
      </AssistantRuntimeAdapter>
    );

    expect(container.querySelector('[data-assistant-content-adapter="reasoning"]')).not.toBeNull();
    expect(
      container.querySelector(
        '[data-assistant-content-adapter="tool-use"][data-tool-call-id="tool-1"]'
      )
    ).not.toBeNull();
    expect(
      container.querySelector(
        '[data-assistant-content-adapter="tool-result"][data-tool-call-id="tool-1"]'
      )
    ).toBeNull();
    expect(
      container.querySelector(
        '[data-assistant-content-adapter="tool-result"][data-tool-call-id="orphan-tool"][data-tool-error="true"]'
      )
    ).not.toBeNull();
  });
});
