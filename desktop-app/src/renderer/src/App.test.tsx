// @vitest-environment jsdom

import { act, createElement, type ReactNode } from 'react'
import { createRoot, type Root } from 'react-dom/client'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

globalThis.IS_REACT_ACT_ENVIRONMENT = true

const threadMessageState = vi.hoisted(() => ({
  message: {
    composer: {
      isEditing: false
    },
    role: 'user'
  }
}))

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

vi.mock('./hooks/useDasclawAssistantRuntime', async (importOriginal) => {
  const actual = await importOriginal<typeof import('./hooks/useDasclawAssistantRuntime')>()
  return {
    ...actual,
    useDasclawAssistantRuntime: () => ({ runtime: {} })
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

vi.mock('@assistant-ui/react', () => {
  const assistantState = {
    composer: {
      dictation: null,
      isEmpty: true
    },
    message: {
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
      const visible = typeof condition === 'function' ? condition(assistantState) : condition
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
      Parts: primitive('Message.Parts'),
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
    unstable_defaultDirectiveFormatter: {},
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
    useAuiState: (selector: (state: typeof assistantState) => unknown) => selector(assistantState)
  }
})

import App from './App'

describe('App composer', () => {
  let container: HTMLDivElement
  let root: Root

  beforeEach(() => {
    threadMessageState.message.composer.isEditing = false
    container = document.createElement('div')
    document.body.appendChild(container)
    root = createRoot(container)
  })

  afterEach(() => {
    act(() => {
      root.unmount()
    })
    container.remove()
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
})
