// @vitest-environment jsdom

import { act, createElement, type ReactNode } from 'react'
import { createRoot, type Root } from 'react-dom/client'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

globalThis.IS_REACT_ACT_ENVIRONMENT = true

import type {
  AppServerDynamicToolCallResponse,
  AppServerServerRequest
} from '../../shared/appServerApi'

type MockThreadMessageState = {
  message: {
    composer: {
      isEditing: boolean
    }
    content: Array<{ type: 'reasoning' | 'text'; text: string }>
    role: 'assistant' | 'user'
    status: { type: 'complete' } | { type: 'running' }
  }
}

const threadMessageState = vi.hoisted<MockThreadMessageState>(() => ({
  message: {
    composer: {
      isEditing: false
    },
    content: [{ type: 'text', text: '正在思考' }],
    role: 'user',
    status: { type: 'complete' }
  }
}))

const streamdownPropsState = vi.hoisted<{
  lastProps: Record<string, unknown> | null
}>(() => ({
  lastProps: null
}))

const runtimeState = vi.hoisted<{
  rejectServerRequest: ReturnType<typeof vi.fn>
  respondToServerRequest: ReturnType<typeof vi.fn>
  serverRequests: AppServerServerRequest[]
}>(() => ({
  rejectServerRequest: vi.fn(),
  respondToServerRequest: vi.fn(),
  serverRequests: []
}))

function resetThreadMessageState(): void {
  threadMessageState.message.composer.isEditing = false
  threadMessageState.message.content = [{ type: 'text', text: '正在思考' }]
  threadMessageState.message.role = 'user'
  threadMessageState.message.status = { type: 'complete' }
  streamdownPropsState.lastProps = null
  runtimeState.rejectServerRequest.mockReset()
  runtimeState.rejectServerRequest.mockResolvedValue(undefined)
  runtimeState.respondToServerRequest.mockReset()
  runtimeState.respondToServerRequest.mockResolvedValue(undefined)
  runtimeState.serverRequests = []
}

function setDesktopPlatform(platform: NodeJS.Platform): void {
  Object.defineProperty(window, 'electron', {
    configurable: true,
    value: {
      process: {
        platform
      }
    }
  })
}

type PrimitiveProps = {
  children?: ReactNode | ((value: unknown) => ReactNode)
  asChild?: boolean
  components?: Record<string, unknown>
  condition?: ((state: unknown) => boolean) | boolean
  char?: string
  placeholder?: string
  directiveChip?: unknown
  className?: string
}

function messagePartComponentFor(
  part: { type: 'reasoning' | 'text' },
  components: Record<string, unknown> | undefined
): React.ComponentType<{ text: string }> | undefined {
  if (part.type === 'reasoning' && typeof components?.Reasoning === 'function') {
    return components.Reasoning as React.ComponentType<{ text: string }>
  }
  if (part.type === 'text' && typeof components?.Text === 'function') {
    return components.Text as React.ComponentType<{ text: string }>
  }
  return undefined
}

vi.mock('./hooks/useDasclawAssistantRuntime', async (importOriginal) => {
  const actual = await importOriginal<typeof import('./hooks/useDasclawAssistantRuntime')>()
  return {
    ...actual,
    useDasclawAssistantRuntime: () => ({
      runtime: {},
      serverRequests: runtimeState.serverRequests,
      respondToServerRequest: runtimeState.respondToServerRequest,
      rejectServerRequest: runtimeState.rejectServerRequest
    })
  }
})

vi.mock('@assistant-ui/react-lexical', () => ({
  LexicalComposerInput: ({ placeholder, directiveChip, className }: PrimitiveProps) => (
    <div
      className={className}
      data-has-directive-chip={String(Boolean(directiveChip))}
      data-placeholder={placeholder}
      data-testid="lexical-composer-input"
    />
  )
}))

vi.mock('@assistant-ui/react-streamdown', () => ({
  StreamdownTextPrimitive: (props: Record<string, unknown>) => {
    streamdownPropsState.lastProps = props
    return <div data-testid="streamdown-text" />
  }
}))

vi.mock('@streamdown/code', () => ({
  code: { plugin: 'code' }
}))

vi.mock('@streamdown/math', () => ({
  math: { plugin: 'math' }
}))

vi.mock('@streamdown/mermaid', () => ({
  mermaid: { plugin: 'mermaid' }
}))

vi.mock('@streamdown/cjk', () => ({
  cjk: { plugin: 'cjk' }
}))

vi.mock('@assistant-ui/react', () => {
  const assistantState = {
    composer: {
      dictation: null,
      isEmpty: true
    },
    message: {
      ...threadMessageState.message,
      isCopied: false
    },
    thread: {
      capabilities: {
        dictation: false
      },
      isLoading: false,
      isRunning: false,
      messages: []
    },
    threads: {
      isLoading: false,
      mainThreadId: 'main',
      threadItems: []
    }
  }

  const currentAssistantState = (): typeof assistantState => ({
    ...assistantState,
    message: {
      ...threadMessageState.message,
      isCopied: false
    }
  })

  const renderChildren = (children: PrimitiveProps['children']): ReactNode => {
    if (typeof children === 'function') return children({ message: { role: 'assistant' } })
    return children
  }

  const omitPrimitiveOnlyProps = (props: PrimitiveProps): Record<string, unknown> => {
    const elementProps = { ...props } as Record<string, unknown>
    delete elementProps.children
    delete elementProps.asChild
    return elementProps
  }

  const primitive = (name: string) => {
    return function Primitive(props: PrimitiveProps): React.JSX.Element {
      return createElement(
        'div',
        { 'data-primitive': name, ...omitPrimitiveOnlyProps(props) },
        renderChildren(props.children)
      )
    }
  }

  return {
    ActionBarPrimitive: {
      Copy: primitive('ActionBar.Copy'),
      Edit: primitive('ActionBar.Edit'),
      Reload: primitive('ActionBar.Reload'),
      Root: primitive('ActionBar.Root')
    },
    AssistantRuntimeProvider: primitive('AssistantRuntimeProvider'),
    AttachmentPrimitive: {
      Name: primitive('Attachment.Name'),
      Root: primitive('Attachment.Root'),
      unstable_Thumb: primitive('Attachment.Thumb')
    },
    AuiIf: ({ children, condition }: PrimitiveProps) => {
      const visible =
        typeof condition === 'function' ? condition(currentAssistantState()) : condition
      return visible ? <>{renderChildren(children)}</> : null
    },
    BranchPickerPrimitive: {
      Count: primitive('BranchPicker.Count'),
      Next: primitive('BranchPicker.Next'),
      Number: primitive('BranchPicker.Number'),
      Previous: primitive('BranchPicker.Previous'),
      Root: primitive('BranchPicker.Root')
    },
    ComposerPrimitive: {
      Cancel: primitive('Composer.Cancel'),
      Input: (props: PrimitiveProps) => (
        <textarea data-testid="plain-composer-input" {...omitPrimitiveOnlyProps(props)} />
      ),
      Root: primitive('Composer.Root'),
      Send: primitive('Composer.Send'),
      Unstable_TriggerPopover: Object.assign(
        ({ char, children }: PrimitiveProps) => (
          <div data-testid="composer-trigger-popover" data-trigger-char={char}>
            {renderChildren(children)}
          </div>
        ),
        {
          Action: () => null,
          Directive: () => null
        }
      ),
      Unstable_TriggerPopoverBack: primitive('Composer.TriggerPopoverBack'),
      Unstable_TriggerPopoverCategories: ({ children }: PrimitiveProps) => (
        <div data-primitive="Composer.TriggerPopoverCategories">
          {typeof children === 'function' ? children([]) : children}
        </div>
      ),
      Unstable_TriggerPopoverCategoryItem: primitive('Composer.TriggerPopoverCategoryItem'),
      Unstable_TriggerPopoverItem: primitive('Composer.TriggerPopoverItem'),
      Unstable_TriggerPopoverItems: ({ children }: PrimitiveProps) => (
        <div data-primitive="Composer.TriggerPopoverItems">
          {typeof children === 'function' ? children([]) : children}
        </div>
      ),
      Unstable_TriggerPopoverRoot: primitive('Composer.TriggerPopoverRoot')
    },
    MessagePrimitive: {
      Attachments: primitive('Message.Attachments'),
      Content: primitive('Message.Content'),
      Error: primitive('Message.Error'),
      Parts: ({ components }: PrimitiveProps) => {
        return (
          <div data-primitive="Message.Parts">
            {threadMessageState.message.content.map((part, index) => {
              const Component = messagePartComponentFor(part, components)
              return Component
                ? createElement(Component, {
                    ...part,
                    key: index
                  })
                : null
            })}
          </div>
        )
      },
      Quote: primitive('Message.Quote'),
      Root: primitive('Message.Root')
    },
    ThreadListItemPrimitive: {
      Archive: primitive('ThreadListItem.Archive'),
      Delete: primitive('ThreadListItem.Delete'),
      Root: primitive('ThreadListItem.Root'),
      Title: primitive('ThreadListItem.Title'),
      Trigger: primitive('ThreadListItem.Trigger')
    },
    ThreadListItemMorePrimitive: {
      Content: primitive('ThreadListItemMore.Content'),
      Item: primitive('ThreadListItemMore.Item'),
      Root: primitive('ThreadListItemMore.Root'),
      Trigger: primitive('ThreadListItemMore.Trigger')
    },
    ThreadListPrimitive: {
      Items: primitive('ThreadList.Items'),
      New: primitive('ThreadList.New'),
      Root: primitive('ThreadList.Root')
    },
    ThreadPrimitive: {
      Messages: ({ children }: PrimitiveProps) => (
        <div data-primitive="Thread.Messages">
          {typeof children === 'function' ? children(threadMessageState) : children}
        </div>
      ),
      Root: primitive('Thread.Root'),
      ScrollToBottom: primitive('Thread.ScrollToBottom'),
      Viewport: primitive('Thread.Viewport'),
      ViewportFooter: primitive('Thread.ViewportFooter')
    },
    unstable_defaultDirectiveFormatter: {
      parse: (text: string) => [{ kind: 'text', text }]
    },
    unstable_useMentionAdapter: () => ({ adapter: {}, directive: {} }),
    unstable_useSlashCommandAdapter: () => ({ action: { onExecute: vi.fn() }, adapter: {} }),
    useAui: () => ({
      composer: () => ({
        getState: () => ({ runConfig: undefined })
      }),
      modelContext: () => ({
        register: vi.fn(() => vi.fn())
      }),
      thread: () => ({
        append: vi.fn(),
        getState: () => ({ isRunning: false })
      })
    }),
    useMessageTiming: () => null,
    useAuiState: (selector: (state: typeof assistantState) => unknown) =>
      selector(currentAssistantState())
  }
})

import App from './App'

describe('App composer', () => {
  let container: HTMLDivElement
  let root: Root

  beforeEach(() => {
    resetThreadMessageState()
    setDesktopPlatform('darwin')
    container = document.createElement('div')
    document.body.appendChild(container)
    root = createRoot(container)
  })

  afterEach(() => {
    act(() => {
      root.unmount()
    })
    container.remove()
    vi.unstubAllGlobals()
  })

  it('uses the Lexical composer input with mention and slash trigger popovers', () => {
    act(() => {
      root.render(<App />)
    })

    const lexicalInput = container.querySelector('[data-testid="lexical-composer-input"]')
    const triggerChars = Array.from(
      container.querySelectorAll('[data-testid="composer-trigger-popover"]')
    )
      .map((node) => node.getAttribute('data-trigger-char'))
      .sort()

    expect(lexicalInput).not.toBeNull()
    expect(lexicalInput?.getAttribute('data-has-directive-chip')).toBe('true')
    expect(lexicalInput?.getAttribute('data-placeholder')).toContain('@')
    expect(container.querySelector('[data-slot="aui_composer-shell"]')?.className).toContain(
      'bg-background'
    )
    expect(container.querySelector('[data-slot="aui_composer-shell"]')?.className).toContain(
      'dark:bg-muted/30'
    )
    expect(container.querySelector('[data-testid="plain-composer-input"]')).toBeNull()
    expect(triggerChars).toEqual(['/', '@'])
  })

  it('renders thread list item actions for archive and delete', () => {
    act(() => {
      root.render(<App />)
    })

    expect(container.querySelector('[data-primitive="ThreadListItemMore.Trigger"]')).not.toBeNull()
    expect(container.querySelector('[data-primitive="ThreadListItem.Archive"]')).not.toBeNull()
    expect(container.querySelector('[data-primitive="ThreadListItem.Delete"]')).not.toBeNull()
  })

  it('renders the sidebar with translucent glass styling', () => {
    act(() => {
      root.render(<App />)
    })

    const sidebar = container.querySelector('[data-slot="app-server-sidebar"]')
    const mainSection = container.querySelector('[data-slot="app-main-section"]')
    const newThread = Array.from(container.querySelectorAll('button')).find(
      (button) => button.textContent?.trim() === 'New thread'
    )
    const threadItem = container.querySelector('[data-primitive="ThreadListItem.Root"]')

    expect(sidebar?.className).toContain('bg-background/50')
    expect(sidebar?.className).toContain('backdrop-blur-xl')
    expect(mainSection?.className).toContain('bg-background/50')
    expect(mainSection?.className).toContain('backdrop-blur-xl')
    expect(sidebar?.className).not.toContain('border-r')
    expect(sidebar?.className).toContain(
      '[@media(prefers-reduced-transparency:reduce)]:backdrop-blur-none'
    )
    expect(newThread?.className).toContain('hover:bg-background/40')
    expect(threadItem?.className).toContain('data-[active]:bg-background/50')
  })

  it('keeps the original opaque sidebar colors on Windows', () => {
    setDesktopPlatform('win32')

    act(() => {
      root.render(<App />)
    })

    const appShell = container.querySelector('main')
    const sidebar = container.querySelector('[data-slot="app-server-sidebar"]')
    const mainSection = container.querySelector('[data-slot="app-main-section"]')
    const newThread = Array.from(container.querySelectorAll('button')).find(
      (button) => button.textContent?.trim() === 'New thread'
    )
    const threadItem = container.querySelector('[data-primitive="ThreadListItem.Root"]')

    expect(appShell?.className).toContain('bg-muted/30')
    expect(sidebar?.className).not.toContain('bg-background/50')
    expect(sidebar?.className).not.toContain('backdrop-blur-xl')
    expect(mainSection?.className).not.toContain('bg-background/50')
    expect(mainSection?.className).not.toContain('backdrop-blur-xl')
    expect(newThread?.className).toContain('hover:bg-muted')
    expect(newThread?.className).not.toContain('hover:bg-background/40')
    expect(threadItem?.className).toContain('data-[active]:bg-muted')
    expect(threadItem?.className).not.toContain('data-[active]:bg-background/50')
  })

  it('renders user messages with the assistant-ui base message structure', () => {
    act(() => {
      root.render(<App />)
    })

    expect(container.querySelector('[data-primitive="Message.Attachments"]')).not.toBeNull()
    expect(container.querySelector('.aui-user-message-content-wrapper')).not.toBeNull()
    expect(container.querySelector('.aui-user-message-content')).not.toBeNull()
    expect(container.querySelector('[data-primitive="Message.Quote"]')).not.toBeNull()
    expect(container.querySelector('[data-primitive="Message.Parts"]')).not.toBeNull()
    expect(container.querySelector('.aui-user-action-bar-wrapper')).not.toBeNull()
    expect(container.querySelector('.aui-user-action-bar-root')).not.toBeNull()
  })

  it('renders the edit composer when a user message enters editing state', () => {
    threadMessageState.message.composer.isEditing = true

    act(() => {
      root.render(<App />)
    })

    expect(container.querySelector('[data-slot="aui_edit-composer-wrapper"]')).not.toBeNull()
    expect(container.querySelector('.aui-edit-composer-root')).not.toBeNull()
    expect(container.querySelector('.aui-user-message-content-wrapper')).toBeNull()
  })

  it('adds shimmer styling to the pending assistant thinking message', () => {
    threadMessageState.message.role = 'assistant'
    threadMessageState.message.status = { type: 'running' }

    act(() => {
      root.render(<App />)
    })

    const assistantContent = container.querySelector('[data-slot="aui_assistant-message-content"]')

    expect(assistantContent?.className).toContain('shimmer')
    expect(assistantContent?.className).toContain('text-foreground/60')
    expect(assistantContent?.className).toContain('motion-reduce:animate-none')
    expect(container.querySelector('[data-slot="aui_assistant-message-footer"]')).toBeNull()
  })

  it('renders assistant text with the streamdown markdown renderer', () => {
    threadMessageState.message.role = 'assistant'
    threadMessageState.message.content = [{ type: 'text', text: '# 标题\n\n- 条目' }]

    act(() => {
      root.render(<App />)
    })

    expect(container.querySelector('[data-testid="streamdown-text"]')).not.toBeNull()
    expect(streamdownPropsState.lastProps).toMatchObject({
      caret: 'block',
      defer: true,
      plugins: {
        code: { plugin: 'code' },
        math: { plugin: 'math' },
        mermaid: { plugin: 'mermaid' },
        cjk: { plugin: 'cjk' }
      }
    })
  })

  it('shows reasoning summary content instead of the thinking placeholder once reasoning streams', () => {
    threadMessageState.message.role = 'assistant'
    threadMessageState.message.status = { type: 'running' }
    threadMessageState.message.content = [{ type: 'reasoning', text: '正在整理上下文' }]

    act(() => {
      root.render(<App />)
    })

    const assistantContent = container.querySelector('[data-slot="aui_assistant-message-content"]')
    const reasoning = container.querySelector('[data-slot="aui_reasoning-part"]')

    expect(assistantContent?.className).not.toContain('shimmer')
    expect(reasoning?.textContent).toContain('推理摘要')
    expect(reasoning?.textContent).toContain('正在整理上下文')
  })

  it('does not render the server request panel when there is no queued request', () => {
    act(() => {
      root.render(<App />)
    })

    expect(container.querySelector('[data-slot="server-request-panel"]')).toBeNull()
  })

  it('responds to a file-change request when accepting file changes', async () => {
    const request = fileChangeApprovalRequest('file-request-1')
    runtimeState.serverRequests = [request]

    act(() => {
      root.render(<App />)
    })

    const accept = buttonWithText('Accept file changes')
    expect(accept).not.toBeUndefined()

    await act(async () => {
      accept?.click()
    })

    expect(runtimeState.respondToServerRequest).toHaveBeenCalledWith(request, {
      decision: 'accept'
    })
  })

  it('responds to a permissions request when allowing once', async () => {
    const request = permissionsApprovalRequest('permissions-request-1')
    runtimeState.serverRequests = [request]

    act(() => {
      root.render(<App />)
    })

    const allow = buttonWithText('Allow once')
    expect(allow).not.toBeUndefined()

    await act(async () => {
      allow?.click()
    })

    expect(runtimeState.respondToServerRequest).toHaveBeenCalledWith(request, {
      decision: 'approve',
      permissions: ['net:fetch'],
      scope: 'turn',
      strictAutoReview: true
    })
  })

  it('runs a client tool request from the panel and responds with sanitized output', async () => {
    const request = toolCallRequest('tool-request-1', {
      url: 'https://example.test/page?token=secret#fragment'
    })
    const openExternalHttpUrl = vi.fn().mockResolvedValue(undefined)
    runtimeState.serverRequests = [request]
    window.desktopAppServer = {
      checkHealth: vi.fn().mockResolvedValue({}),
      getStatus: vi.fn().mockResolvedValue({}),
      onNotification: vi.fn(() => vi.fn()),
      onStatusChange: vi.fn(() => vi.fn()),
      openExternalHttpUrl,
      request: vi.fn().mockResolvedValue({ models: [] }),
      respondServerRequest: vi.fn().mockResolvedValue(undefined),
      stop: vi.fn().mockResolvedValue({})
    } as Window['desktopAppServer']

    act(() => {
      root.render(<App />)
    })

    const run = buttonWithText('Run client tool')
    expect(run).not.toBeUndefined()

    await act(async () => {
      run?.click()
    })

    const response = {
      success: true,
      contentItems: [{ type: 'inputText', text: 'Opened URL' }]
    } satisfies AppServerDynamicToolCallResponse
    expect(openExternalHttpUrl).toHaveBeenCalledWith(
      'https://example.test/page?token=secret#fragment'
    )
    expect(runtimeState.respondToServerRequest).toHaveBeenCalledWith(request, response)
  })
})

function buttonWithText(text: string): HTMLButtonElement | undefined {
  return Array.from(document.querySelectorAll('button')).find(
    (button) => button.textContent?.trim() === text
  )
}

function fileChangeApprovalRequest(
  requestId: string
): AppServerServerRequest<'item/fileChange/requestApproval'> {
  return {
    hostId: 'local',
    requestId,
    method: 'item/fileChange/requestApproval',
    params: {
      threadId: 'thread_1',
      turnId: 'turn_1',
      itemId: 'file_1',
      reason: 'modify src/App.tsx',
      grantRoot: '/workspace'
    }
  }
}

function permissionsApprovalRequest(
  requestId: string
): AppServerServerRequest<'item/permissions/requestApproval'> {
  return {
    hostId: 'local',
    requestId,
    method: 'item/permissions/requestApproval',
    params: {
      threadId: 'thread_1',
      turnId: 'turn_1',
      itemId: 'permission_1',
      cwd: '/workspace',
      reason: 'needs network',
      permissions: ['net:fetch']
    }
  }
}

function toolCallRequest(
  requestId: string,
  args: unknown
): AppServerServerRequest<'item/tool/call'> {
  return {
    hostId: 'local',
    requestId,
    method: 'item/tool/call',
    params: {
      threadId: 'thread_1',
      turnId: 'turn_1',
      callId: 'call_1',
      namespace: 'client',
      tool: 'open_url',
      arguments: args
    }
  }
}
