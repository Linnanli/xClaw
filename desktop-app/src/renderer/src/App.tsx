import {
  ActionBarPrimitive,
  AssistantRuntimeProvider,
  AttachmentPrimitive,
  AuiIf,
  BranchPickerPrimitive,
  ComposerPrimitive,
  type AssistantState,
  MessagePrimitive,
  ThreadListItemMorePrimitive,
  ThreadListItemPrimitive,
  ThreadListPrimitive,
  ThreadPrimitive,
  type Unstable_DirectiveFormatter,
  type QuoteMessagePartProps,
  type ReasoningMessagePartProps,
  type TextMessagePartProps,
  type Unstable_SlashCommand,
  type Unstable_TriggerItem,
  unstable_defaultDirectiveFormatter,
  unstable_useMentionAdapter,
  unstable_useSlashCommandAdapter,
  useAui,
  useAuiState
} from '@assistant-ui/react'
import { LexicalComposerInput, type DirectiveChipProps } from '@assistant-ui/react-lexical'
import { StreamdownTextPrimitive } from '@assistant-ui/react-streamdown'
import { cjk } from '@streamdown/cjk'
import { code } from '@streamdown/code'
import { math } from '@streamdown/math'
import { mermaid } from '@streamdown/mermaid'
import { MessageTiming } from '@/components/assistant-ui/message-timing'
import {
  ActivityIcon,
  ArchiveIcon,
  ArrowDownIcon,
  ArrowUpIcon,
  CheckIcon,
  ChevronLeftIcon,
  ChevronRightIcon,
  CopyIcon,
  FileTextIcon,
  HelpCircleIcon,
  MoreHorizontalIcon,
  PanelLeftIcon,
  PencilIcon,
  PlusIcon,
  QuoteIcon,
  SlashIcon,
  SquareIcon,
  TrashIcon,
  WrenchIcon
} from 'lucide-react'
import {
  forwardRef,
  useState,
  type ButtonHTMLAttributes,
  type ComponentPropsWithoutRef,
  type FC,
  type ReactNode
} from 'react'

import { ModelSelector } from './components/assistant-ui'
import { ServerRequestPanel } from './components/assistant-ui/server-request-panel'
import { cn } from './lib/utils'
import { isPendingAssistantMessageContent } from './lib/assistantMessages'
import {
  useAppServerModelSelectorState,
  useDasclawAssistantRuntime
} from './hooks/useDasclawAssistantRuntime'

type AppServerSidebarProps = {
  collapsed: boolean
  nativeBackdrop: boolean
}

type HeaderProps = {
  sidebarCollapsed: boolean
  onToggleSidebar: () => void
}

type IconButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  label: string
}

type IconComponent = FC<{ className?: string }>

type DirectiveBehaviorProps = {
  formatter?: Unstable_DirectiveFormatter
  onInserted?: (item: Unstable_TriggerItem) => void
}

type ActionBehaviorProps = {
  formatter?: Unstable_DirectiveFormatter
  onExecute: (item: Unstable_TriggerItem) => void
  removeOnExecute?: boolean
}

type ComposerTriggerPopoverBaseProps = Omit<
  ComponentPropsWithoutRef<typeof ComposerPrimitive.Unstable_TriggerPopover>,
  'children'
> & {
  backLabel?: string
  emptyCategoriesLabel?: string
  emptyItemsLabel?: string
  fallbackIcon?: IconComponent
  iconMap?: Record<string, IconComponent>
}

type ComposerTriggerPopoverProps = ComposerTriggerPopoverBaseProps &
  (
    | {
        action?: never
        directive: DirectiveBehaviorProps
      }
    | {
        action: ActionBehaviorProps
        directive?: never
      }
  )

const noopSlashCommand = (): void => {}

const slashCommands: readonly Unstable_SlashCommand[] = [
  {
    id: 'explain-changes',
    label: '解释改动',
    description: '总结当前工作区里的主要变化',
    icon: 'FileText',
    execute: noopSlashCommand
  },
  {
    id: 'draft-pr',
    label: '生成 PR 描述',
    description: '整理背景、范围和验证信息',
    icon: 'Pencil',
    execute: noopSlashCommand
  },
  {
    id: 'review-risks',
    label: '审查风险',
    description: '查找潜在回归和遗漏测试',
    icon: 'HelpCircle',
    execute: noopSlashCommand
  }
]

const slashIconMap: Record<string, IconComponent> = {
  FileText: FileTextIcon,
  HelpCircle: HelpCircleIcon,
  Pencil: PencilIcon
}

const streamdownPlugins = { code, math, mermaid, cjk }

const sidebarBaseClass =
  'hidden h-full shrink-0 flex-col overflow-hidden transition-all duration-200 md:flex'

const nativeBackdropSurfaceClass =
  'bg-background/50 bg-clip-padding backdrop-blur-xl [@media(prefers-reduced-transparency:reduce)]:bg-background [@media(prefers-reduced-transparency:reduce)]:backdrop-blur-none dark:bg-background/30'

const sidebarGlassClass =
  'shadow-[0_18px_60px_-48px_rgba(15,23,42,0.75)] dark:shadow-[0_18px_60px_-48px_rgba(0,0,0,0.95)]'

const threadListNewButtonClass =
  'inline-flex h-8 w-full items-center gap-2 rounded-md px-3 text-sm font-medium text-foreground transition-colors'

const threadListNewButtonGlassClass = 'hover:bg-background/40 dark:hover:bg-foreground/8'

const threadListItemClass = 'group flex min-h-8 items-center gap-1 rounded-md transition-colors'

const threadListItemGlassClass =
  'hover:bg-background/40 focus-visible:bg-background/40 data-[active]:bg-background/50 dark:hover:bg-foreground/8 dark:focus-visible:bg-foreground/8 dark:data-[active]:bg-foreground/10'

function useNativeBackdrop(): boolean {
  return window.electron?.process.platform === 'darwin'
}

function App(): React.JSX.Element {
  const { runtime, serverRequests, respondToServerRequest, rejectServerRequest } =
    useDasclawAssistantRuntime()
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false)
  const nativeBackdrop = useNativeBackdrop()

  const toggleSidebar = (): void => {
    setSidebarCollapsed((collapsed) => !collapsed)
  }

  return (
    <AssistantRuntimeProvider runtime={runtime}>
      <main
        className={cn(
          'flex h-screen w-full text-foreground',
          nativeBackdrop ? 'bg-background/10 dark:bg-background/10' : 'bg-muted/30'
        )}
      >
        <AppServerSidebar collapsed={sidebarCollapsed} nativeBackdrop={nativeBackdrop} />
        <section
          data-slot="app-main-section"
          className={cn(
            'flex min-w-0 flex-1 flex-col overflow-hidden p-2 transition-[padding] duration-200',
            nativeBackdrop && nativeBackdropSurfaceClass,
            !sidebarCollapsed && 'md:pl-0'
          )}
        >
          <div className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-lg border border-border/50 bg-background shadow-[0_18px_60px_-48px_rgba(15,23,42,0.75)]">
            <Header sidebarCollapsed={sidebarCollapsed} onToggleSidebar={toggleSidebar} />
            <div className="min-h-0 flex-1 overflow-hidden">
              <ChatThread />
            </div>
            <ServerRequestPanel
              onReject={rejectServerRequest}
              onRespond={respondToServerRequest}
              requests={serverRequests}
            />
          </div>
        </section>
      </main>
    </AssistantRuntimeProvider>
  )
}

function AppServerSidebar({ collapsed, nativeBackdrop }: AppServerSidebarProps): React.JSX.Element {
  return (
    <aside
      data-slot="app-server-sidebar"
      className={cn(
        sidebarBaseClass,
        nativeBackdrop && nativeBackdropSurfaceClass,
        nativeBackdrop && sidebarGlassClass,
        collapsed ? 'w-12' : 'w-65'
      )}
    >
      {collapsed ? (
        <div className="flex flex-col items-center gap-1">
          <div className="mt-2 flex h-12 shrink-0 items-center justify-center">
            <BrandMark />
          </div>
          <ThreadListPrimitive.New asChild>
            <IconButton className="size-8" label="新对话" title="新对话">
              <PlusIcon className="size-4" />
            </IconButton>
          </ThreadListPrimitive.New>
        </div>
      ) : (
        <>
          <div className="mt-2 flex h-12 shrink-0 items-center px-4">
            <Logo />
          </div>
          <div className="relative min-h-0 flex-1 overflow-y-auto p-3">
            <ThreadList nativeBackdrop={nativeBackdrop} />
          </div>
        </>
      )}
    </aside>
  )
}

function Logo(): React.JSX.Element {
  return (
    <div className="flex min-w-0 items-center gap-2 px-2 text-sm font-medium">
      <BrandMark />
      <span className="min-w-0 truncate text-foreground/90">Dasclaw</span>
    </div>
  )
}

function BrandMark(): React.JSX.Element {
  return (
    <div className="grid size-5 shrink-0 place-items-center rounded-md bg-primary text-[11px] font-bold text-primary-foreground">
      D
    </div>
  )
}

function ThreadList({ nativeBackdrop }: { nativeBackdrop: boolean }): React.JSX.Element {
  return (
    <ThreadListPrimitive.Root className="flex flex-col gap-1">
      <ThreadListPrimitive.New asChild>
        <button
          className={cn(
            threadListNewButtonClass,
            nativeBackdrop ? threadListNewButtonGlassClass : 'hover:bg-muted'
          )}
          type="button"
        >
          <PlusIcon className="size-4" />
          New thread
        </button>
      </ThreadListPrimitive.New>
      <ThreadListPrimitive.Items>
        {() => <ThreadListItem nativeBackdrop={nativeBackdrop} />}
      </ThreadListPrimitive.Items>
    </ThreadListPrimitive.Root>
  )
}

function ThreadListItem({ nativeBackdrop }: { nativeBackdrop: boolean }): React.JSX.Element {
  return (
    <ThreadListItemPrimitive.Root
      className={cn(
        threadListItemClass,
        nativeBackdrop
          ? threadListItemGlassClass
          : 'hover:bg-muted focus-visible:bg-muted data-[active]:bg-muted'
      )}
    >
      <ThreadListItemPrimitive.Trigger className="flex min-w-0 flex-1 items-center px-3 text-left text-sm font-medium text-foreground outline-none">
        <span className="min-w-0 flex-1 truncate">
          <ThreadListItemPrimitive.Title fallback="New Chat" />
        </span>
      </ThreadListItemPrimitive.Trigger>
      <ThreadListItemActions />
    </ThreadListItemPrimitive.Root>
  )
}

function ThreadListItemActions(): React.JSX.Element {
  return (
    <ThreadListItemMorePrimitive.Root>
      <ThreadListItemMorePrimitive.Trigger asChild>
        <IconButton
          className="mr-1.5 size-6 opacity-0 transition-opacity group-hover:opacity-100 group-data-[active]:opacity-100 data-[state=open]:bg-accent data-[state=open]:opacity-100"
          label="更多线程选项"
          title="更多线程选项"
        >
          <MoreHorizontalIcon className="size-3.5" />
        </IconButton>
      </ThreadListItemMorePrimitive.Trigger>
      <ThreadListItemMorePrimitive.Content
        align="start"
        className="z-50 min-w-32 overflow-hidden rounded-lg border border-border bg-popover p-1 text-popover-foreground shadow-lg"
        side="right"
        sideOffset={6}
      >
        <ThreadListItemPrimitive.Archive asChild>
          <ThreadListItemMorePrimitive.Item className="flex cursor-pointer items-center gap-2 rounded-md px-2.5 py-1.5 text-sm outline-none select-none hover:bg-accent hover:text-accent-foreground focus:bg-accent focus:text-accent-foreground">
            <ArchiveIcon className="size-4" />
            Archive
          </ThreadListItemMorePrimitive.Item>
        </ThreadListItemPrimitive.Archive>
        <ThreadListItemPrimitive.Delete asChild>
          <ThreadListItemMorePrimitive.Item className="flex cursor-pointer items-center gap-2 rounded-md px-2.5 py-1.5 text-sm text-destructive outline-none select-none hover:bg-destructive/10 hover:text-destructive focus:bg-destructive/10 focus:text-destructive">
            <TrashIcon className="size-4" />
            Delete
          </ThreadListItemMorePrimitive.Item>
        </ThreadListItemPrimitive.Delete>
      </ThreadListItemMorePrimitive.Content>
    </ThreadListItemMorePrimitive.Root>
  )
}

function Header({ sidebarCollapsed, onToggleSidebar }: HeaderProps): React.JSX.Element {
  const toggleLabel = sidebarCollapsed ? '显示侧栏' : '隐藏侧栏'

  return (
    <header className="flex h-12 shrink-0 items-center gap-2 px-4">
      <IconButton
        className="hidden md:grid"
        label={toggleLabel}
        title={toggleLabel}
        onClick={onToggleSidebar}
      >
        <PanelLeftIcon className="size-4" />
      </IconButton>
      <ThreadTitle />
      <div className="ml-auto" />
    </header>
  )
}

function ThreadTitle(): React.JSX.Element {
  const title = useAuiState(
    (state) =>
      state.threads.threadItems.find((thread) => thread.id === state.threads.mainThreadId)?.title
  )

  return <span className="min-w-0 truncate text-sm font-medium">{title ?? 'New Chat'}</span>
}

function isNewChatView(state: AssistantState): boolean {
  return state.thread.messages.length === 0 && (!state.thread.isLoading || state.threads.isLoading)
}

function ChatThread(): React.JSX.Element {
  const isEmpty = useAuiState(isNewChatView)

  return (
    <ThreadPrimitive.Root
      className="aui-root aui-thread-root @container flex h-full min-h-0 flex-1 flex-col bg-background"
      style={{
        ['--thread-max-width' as string]: '44rem',
        ['--composer-padding' as string]: '8px'
      }}
    >
      <ThreadPrimitive.Viewport
        turnAnchor="top"
        data-slot="aui_thread-viewport"
        className={cn(
          'relative flex flex-1 flex-col overflow-x-auto overflow-y-scroll scroll-smooth px-4 pt-4',
          isEmpty && 'justify-center'
        )}
      >
        <AuiIf condition={isNewChatView}>
          <ThreadWelcome />
        </AuiIf>
        <div data-slot="aui_message-group" className="mb-14 flex flex-col gap-y-6 empty:hidden">
          <ThreadPrimitive.Messages>
            {({ message }) => {
              if (message.composer.isEditing) return <EditComposer />
              if (message.role === 'user') return <UserMessage />
              return <AssistantMessage />
            }}
          </ThreadPrimitive.Messages>
        </div>
        <ThreadPrimitive.ViewportFooter
          className={cn(
            'aui-thread-viewport-footer mx-auto flex w-full max-w-(--thread-max-width) flex-col gap-4 overflow-visible bg-background pb-4 md:pb-6',
            !isEmpty && 'sticky bottom-0 mt-auto rounded-t-xl'
          )}
        >
          <ThreadScrollToBottom />
          <Composer />
          <AuiIf condition={isNewChatView}>
            <div className="aui-thread-welcome-suggestions-shell min-h-19">
              <AuiIf condition={(state) => state.composer.isEmpty}>
                <ThreadSuggestions />
              </AuiIf>
            </div>
          </AuiIf>
        </ThreadPrimitive.ViewportFooter>
      </ThreadPrimitive.Viewport>
    </ThreadPrimitive.Root>
  )
}

function ThreadWelcome(): React.JSX.Element {
  return (
    <section className="aui-thread-welcome-root mx-auto mb-6 flex w-full max-w-(--thread-max-width) flex-col items-center px-4 text-center">
      <h1 className="aui-thread-welcome-message-inner duration-200 animate-in fade-in slide-in-from-bottom-1 text-2xl font-semibold tracking-[-0.02em]">
        How can I help you today?
      </h1>
    </section>
  )
}

type SuggestionGroup = {
  label: string
  icon: ReactNode
  options: { label: string; prompt: string }[]
}

const suggestionGroups: SuggestionGroup[] = [
  {
    label: '代码',
    icon: <PencilIcon size={15} />,
    options: [
      { label: '解释当前改动', prompt: '请解释当前工作区里的主要改动。' },
      { label: '生成 PR 描述', prompt: '请根据当前改动生成一份 PR 描述。' },
      { label: '找潜在风险', prompt: '请审查当前改动里可能的风险。' }
    ]
  },
  {
    label: '任务',
    icon: <ActivityIcon size={15} />,
    options: [
      { label: '列出下一步', prompt: '请根据当前上下文列出最小下一步。' },
      { label: '总结线程', prompt: '请总结这个线程目前的目标和状态。' },
      { label: '整理待办', prompt: '请把当前任务整理成可执行的待办清单。' }
    ]
  }
]

const suggestionChipClass =
  'aui-thread-welcome-suggestion h-auto gap-1.5 rounded-full border border-border/60 px-3.5 py-1.5 text-sm font-normal whitespace-nowrap text-foreground transition-colors hover:bg-muted [&_svg]:size-4'

function ThreadSuggestions(): React.JSX.Element {
  const aui = useAui()
  const [expandedLabel, setExpandedLabel] = useState<string | null>(null)
  const expandedGroup = suggestionGroups.find((group) => group.label === expandedLabel)

  const sendPrompt = (prompt: string): void => {
    if (aui.thread().getState().isRunning) return
    aui.thread().append({
      content: [{ type: 'text', text: prompt }],
      runConfig: aui.composer().getState().runConfig
    })
  }

  const toggleGroup = (label: string): void => {
    setExpandedLabel((currentLabel) => (currentLabel === label ? null : label))
  }

  return (
    <div className="aui-thread-welcome-suggestions flex w-full flex-col gap-2 px-4">
      <div className="w-full overflow-x-auto">
        <div className="mx-auto flex w-max items-center gap-2">
          {suggestionGroups.map((group) => (
            <button
              key={group.label}
              className={cn(suggestionChipClass, group.label === expandedLabel && 'bg-muted')}
              type="button"
              onClick={() => toggleGroup(group.label)}
            >
              {group.icon}
              {group.label}
            </button>
          ))}
        </div>
      </div>
      {expandedGroup ? (
        <div className="w-full overflow-x-auto duration-200 animate-in fade-in slide-in-from-top-1">
          <div className="mx-auto flex w-max items-center gap-2">
            {expandedGroup.options.map((option) => (
              <button
                key={option.label}
                className={suggestionChipClass}
                type="button"
                onClick={() => sendPrompt(option.prompt)}
              >
                {option.label}
              </button>
            ))}
          </div>
        </div>
      ) : null}
    </div>
  )
}

function ThreadScrollToBottom(): React.JSX.Element {
  return (
    <ThreadPrimitive.ScrollToBottom asChild>
      <IconButton
        className="aui-thread-scroll-to-bottom absolute -top-12 z-10 self-center rounded-full border border-border bg-background p-4 shadow-sm disabled:invisible"
        label="滚动到底部"
        title="滚动到底部"
      >
        <ArrowDownIcon className="size-4" />
      </IconButton>
    </ThreadPrimitive.ScrollToBottom>
  )
}

function AssistantMessage(): React.JSX.Element {
  const isThinking = useAuiState(
    (state) =>
      state.message.status?.type === 'running' &&
      isPendingAssistantMessageContent(state.message.content)
  )

  return (
    <MessagePrimitive.Root
      data-slot="aui_assistant-message-root"
      data-role="assistant"
      className="relative mx-auto w-full max-w-(--thread-max-width) duration-150 animate-in fade-in slide-in-from-bottom-1"
    >
      <div
        data-slot="aui_assistant-message-content"
        className={cn(
          'wrap-break-word px-2 leading-relaxed text-foreground',
          isThinking && 'shimmer text-foreground/60 motion-reduce:animate-none'
        )}
      >
        <MessagePrimitive.Parts components={{ Text: AssistantText, Reasoning: ReasoningPart }} />
        <MessagePrimitive.Error />
      </div>
      {isThinking ? null : (
        <div
          data-slot="aui_assistant-message-footer"
          className="ml-2 flex min-h-7.5 items-center pt-1.5 -mb-7.5"
        >
          <BranchPicker />
          <AssistantActionBar />
        </div>
      )}
    </MessagePrimitive.Root>
  )
}

function UserMessage(): React.JSX.Element {
  return (
    <MessagePrimitive.Root
      data-slot="aui_user-message-root"
      data-role="user"
      className="mx-auto grid w-full max-w-(--thread-max-width) auto-rows-auto grid-cols-[minmax(72px,1fr)_auto] content-start gap-y-2 px-2 duration-150 animate-in fade-in slide-in-from-bottom-1 [&:where(>*)]:col-start-2"
    >
      <UserMessageAttachments />

      <div className="aui-user-message-content-wrapper relative col-start-2 min-w-0">
        <div className="aui-user-message-content peer rounded-xl bg-muted px-4 py-2 text-foreground wrap-break-word empty:hidden">
          <MessagePrimitive.Quote>{(quote) => <QuoteBlock {...quote} />}</MessagePrimitive.Quote>
          <MessagePrimitive.Parts components={{ Text: DirectiveText }} />
        </div>
        <div className="aui-user-action-bar-wrapper absolute top-1/2 left-0 -translate-x-full -translate-y-1/2 pr-2 peer-empty:hidden">
          <UserActionBar />
        </div>
      </div>

      <BranchPicker
        data-slot="aui_user-branch-picker"
        className="col-span-full col-start-1 row-start-3 -mr-1 justify-end"
      />
    </MessagePrimitive.Root>
  )
}

function UserMessageAttachments(): React.JSX.Element {
  return (
    <div className="aui-user-message-attachments-end col-span-full col-start-1 row-start-1 flex w-full flex-row justify-end gap-2">
      <MessagePrimitive.Attachments>{() => <UserMessageAttachment />}</MessagePrimitive.Attachments>
    </div>
  )
}

function UserMessageAttachment(): React.JSX.Element {
  return (
    <AttachmentPrimitive.Root className="aui-attachment-root relative">
      <div className="aui-attachment-tile flex size-14 items-center justify-center overflow-hidden rounded-md border bg-muted text-muted-foreground">
        <AttachmentPrimitive.unstable_Thumb className="size-full object-cover" />
        <FileTextIcon className="size-6" />
      </div>
      <span className="sr-only">
        <AttachmentPrimitive.Name />
      </span>
    </AttachmentPrimitive.Root>
  )
}

function QuoteBlock({ text }: QuoteMessagePartProps): React.JSX.Element {
  return (
    <div data-slot="quote-block" className="mb-2 flex items-start gap-1.5">
      <QuoteIcon
        data-slot="quote-block-icon"
        className="mt-0.5 size-3 shrink-0 text-muted-foreground/60"
      />
      <p
        data-slot="quote-block-text"
        className="line-clamp-2 min-w-0 text-sm text-muted-foreground/80 italic"
      >
        {text}
      </p>
    </div>
  )
}

function DirectiveText({ text }: TextMessagePartProps): React.JSX.Element {
  const segments = unstable_defaultDirectiveFormatter.parse(text)

  if (segments.length === 1 && segments[0]?.kind === 'text') return <>{text}</>

  return (
    <>
      {segments.map((segment, index) => {
        if (segment.kind === 'text') {
          return (
            <span key={index} className="whitespace-pre-wrap">
              {segment.text}
            </span>
          )
        }

        return (
          <span
            key={index}
            className="aui-directive-chip inline-flex items-baseline rounded-md bg-blue-100 px-1.5 py-0.5 text-[13px] leading-none font-medium text-blue-700 dark:bg-blue-900/50 dark:text-blue-300"
            data-directive-id={segment.id}
            data-directive-type={segment.type}
          >
            {segment.label}
          </span>
        )
      })}
    </>
  )
}

function AssistantText(): React.JSX.Element {
  return <StreamdownTextPrimitive caret="block" defer plugins={streamdownPlugins} />
}

function ReasoningPart({ text }: ReasoningMessagePartProps): React.JSX.Element {
  return (
    <section
      className="my-2 rounded-md border border-border/70 bg-muted/40 px-3 py-2 text-sm text-muted-foreground"
      data-slot="aui_reasoning-part"
    >
      <div className="font-medium text-foreground">推理摘要</div>
      <div className="mt-2 whitespace-pre-wrap">{text}</div>
    </section>
  )
}

function UserActionBar(): React.JSX.Element {
  return (
    <ActionBarPrimitive.Root
      hideWhenRunning
      autohide="not-last"
      className="aui-user-action-bar-root flex flex-col items-end"
    >
      <ActionBarPrimitive.Edit asChild>
        <IconButton className="aui-user-action-edit" label="编辑" title="编辑">
          <PencilIcon className="size-4" />
        </IconButton>
      </ActionBarPrimitive.Edit>
    </ActionBarPrimitive.Root>
  )
}

function EditComposer(): React.JSX.Element {
  return (
    <MessagePrimitive.Root
      data-slot="aui_edit-composer-wrapper"
      className="mx-auto flex w-full max-w-(--thread-max-width) flex-col px-2"
    >
      <ComposerPrimitive.Unstable_TriggerPopoverRoot>
        <ComposerPrimitive.Root className="aui-edit-composer-root ml-auto flex w-full max-w-[85%] flex-col rounded-3xl border border-border/60 bg-background shadow-[0_4px_16px_-8px_rgba(0,0,0,0.08),0_1px_2px_rgba(0,0,0,0.04)] dark:border-muted-foreground/15 dark:bg-muted/30 dark:shadow-none">
          <LexicalComposerInput
            autoFocus
            directiveChip={DirectiveChip}
            className="aui-edit-composer-input min-h-14 w-full resize-none bg-transparent px-4 pt-3 pb-1 text-base text-foreground outline-none [&_.aui-directive-chip]:inline-flex [&_.aui-directive-chip]:items-baseline [&_.aui-directive-chip]:gap-1 [&_.aui-directive-chip]:rounded-md [&_.aui-directive-chip]:bg-blue-100 [&_.aui-directive-chip]:px-1.5 [&_.aui-directive-chip]:py-0.5 [&_.aui-directive-chip]:text-[13px] [&_.aui-directive-chip]:leading-none [&_.aui-directive-chip]:font-medium [&_.aui-directive-chip]:text-blue-700 [&_.aui-directive-chip-icon]:self-center [&_.aui-lexical-input]:min-h-lh [&_.aui-lexical-input]:outline-none dark:[&_.aui-directive-chip]:bg-blue-900/50 dark:[&_.aui-directive-chip]:text-blue-300"
          />
          <div className="aui-edit-composer-footer mx-2.5 mb-2.5 flex items-center gap-1.5 self-end">
            <ComposerPrimitive.Cancel asChild>
              <button
                className="h-8 rounded-full px-3.5 text-sm text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
                type="button"
              >
                取消
              </button>
            </ComposerPrimitive.Cancel>
            <ComposerPrimitive.Send asChild>
              <button
                className="h-8 rounded-full bg-primary px-3.5 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90"
                type="button"
              >
                更新
              </button>
            </ComposerPrimitive.Send>
          </div>
        </ComposerPrimitive.Root>
      </ComposerPrimitive.Unstable_TriggerPopoverRoot>
    </MessagePrimitive.Root>
  )
}

function AssistantActionBar(): React.JSX.Element {
  return (
    <ActionBarPrimitive.Root
      className="flex items-center gap-1 text-muted-foreground duration-200 animate-in fade-in"
      hideWhenRunning
      autohide="not-last"
    >
      <ActionBarPrimitive.Copy asChild>
        <IconButton label="复制" title="复制">
          <AuiIf condition={(state) => state.message.isCopied}>
            <CheckIcon className="size-4" />
          </AuiIf>
          <AuiIf condition={(state) => !state.message.isCopied}>
            <CopyIcon className="size-4" />
          </AuiIf>
        </IconButton>
      </ActionBarPrimitive.Copy>
      <MessageTiming />
    </ActionBarPrimitive.Root>
  )
}

function BranchPicker({
  className,
  ...props
}: BranchPickerPrimitive.Root.Props): React.JSX.Element {
  return (
    <BranchPickerPrimitive.Root
      hideWhenSingleBranch
      className={cn(
        'inline-flex items-center gap-0.5 text-xs font-medium text-muted-foreground',
        className
      )}
      {...props}
    >
      <BranchPickerPrimitive.Previous asChild>
        <IconButton label="上一条" title="上一条">
          <ChevronLeftIcon className="size-3.5" />
        </IconButton>
      </BranchPickerPrimitive.Previous>
      <span className="px-1">
        <BranchPickerPrimitive.Number /> / <BranchPickerPrimitive.Count />
      </span>
      <BranchPickerPrimitive.Next asChild>
        <IconButton label="下一条" title="下一条">
          <ChevronRightIcon className="size-3.5" />
        </IconButton>
      </BranchPickerPrimitive.Next>
    </BranchPickerPrimitive.Root>
  )
}

function DirectiveChip({
  directiveId,
  directiveType,
  label
}: DirectiveChipProps): React.JSX.Element {
  const showWrench = directiveType !== 'command'

  return (
    <span
      className="aui-directive-chip"
      data-directive-id={directiveId}
      data-directive-type={directiveType}
    >
      {showWrench ? (
        <span className="aui-directive-chip-icon">
          <WrenchIcon className="size-3" />
        </span>
      ) : null}
      <span className="aui-directive-chip-label">{label}</span>
    </span>
  )
}

function resolveTriggerIcon(
  iconKey: string | undefined,
  iconMap: Record<string, IconComponent> | undefined,
  fallbackIcon: IconComponent
): IconComponent {
  if (iconKey && iconMap?.[iconKey]) return iconMap[iconKey]
  return fallbackIcon
}

function TriggerPopoverCategories({
  emptyLabel,
  fallbackIcon,
  iconMap
}: {
  emptyLabel: string
  fallbackIcon: IconComponent
  iconMap?: Record<string, IconComponent>
}): React.JSX.Element {
  return (
    <ComposerPrimitive.Unstable_TriggerPopoverCategories>
      {(categories) => (
        <div className="flex flex-col py-1" data-slot="composer-trigger-popover-categories">
          {categories.map((category) => {
            const Icon = resolveTriggerIcon(category.id, iconMap, fallbackIcon)

            return (
              <ComposerPrimitive.Unstable_TriggerPopoverCategoryItem
                key={category.id}
                categoryId={category.id}
                className="flex cursor-pointer items-center justify-between gap-2 px-3 py-2 text-sm transition-colors outline-none hover:bg-accent focus:bg-accent data-[highlighted]:bg-accent"
              >
                <span className="flex items-center gap-2">
                  <Icon className="size-4 text-muted-foreground" />
                  {category.label}
                </span>
                <ChevronRightIcon className="size-4 text-muted-foreground" />
              </ComposerPrimitive.Unstable_TriggerPopoverCategoryItem>
            )
          })}
          {categories.length === 0 ? (
            <div className="px-3 py-2 text-sm text-muted-foreground">{emptyLabel}</div>
          ) : null}
        </div>
      )}
    </ComposerPrimitive.Unstable_TriggerPopoverCategories>
  )
}

function TriggerPopoverItems({
  backLabel,
  emptyLabel,
  fallbackIcon,
  iconMap
}: {
  backLabel: string
  emptyLabel: string
  fallbackIcon: IconComponent
  iconMap?: Record<string, IconComponent>
}): React.JSX.Element {
  return (
    <ComposerPrimitive.Unstable_TriggerPopoverItems>
      {(items) => (
        <div className="flex flex-col" data-slot="composer-trigger-popover-items">
          <ComposerPrimitive.Unstable_TriggerPopoverBack className="flex cursor-pointer items-center gap-1.5 border-b px-3 py-2 text-xs text-muted-foreground uppercase transition-colors hover:bg-accent">
            <ChevronLeftIcon className="size-3.5" />
            {backLabel}
          </ComposerPrimitive.Unstable_TriggerPopoverBack>

          <div className="py-1">
            {items.map((item, index) => {
              const iconKey =
                typeof item.metadata?.icon === 'string' ? item.metadata.icon : undefined
              const Icon = resolveTriggerIcon(iconKey, iconMap, fallbackIcon)

              return (
                <ComposerPrimitive.Unstable_TriggerPopoverItem
                  key={item.id}
                  item={item}
                  index={index}
                  className="flex w-full cursor-pointer flex-col items-start gap-0.5 px-3 py-2 text-start transition-colors outline-none hover:bg-accent focus:bg-accent data-[highlighted]:bg-accent"
                >
                  <span className="flex items-center gap-2 text-sm font-medium">
                    <Icon className="size-3.5 text-primary" />
                    {item.label}
                  </span>
                  {item.description ? (
                    <span className="ms-5.5 text-xs leading-tight text-muted-foreground">
                      {item.description}
                    </span>
                  ) : null}
                </ComposerPrimitive.Unstable_TriggerPopoverItem>
              )
            })}
            {items.length === 0 ? (
              <div className="px-3 py-2 text-sm text-muted-foreground">{emptyLabel}</div>
            ) : null}
          </div>
        </div>
      )}
    </ComposerPrimitive.Unstable_TriggerPopoverItems>
  )
}

function ComposerTriggerPopover({
  action,
  backLabel = '返回',
  className,
  directive,
  emptyCategoriesLabel = '没有可用项目',
  emptyItemsLabel = '没有匹配项',
  fallbackIcon = SlashIcon,
  iconMap,
  ...props
}: ComposerTriggerPopoverProps): React.JSX.Element {
  return (
    <ComposerPrimitive.Unstable_TriggerPopover
      className={cn(
        'aui-composer-trigger-popover absolute bottom-full start-0 z-50 mb-2 w-64 overflow-hidden rounded-xl border bg-popover text-popover-foreground shadow-lg',
        className
      )}
      data-slot="composer-trigger-popover"
      {...props}
    >
      {directive ? (
        <ComposerPrimitive.Unstable_TriggerPopover.Directive
          formatter={directive.formatter ?? unstable_defaultDirectiveFormatter}
          onInserted={directive.onInserted}
        />
      ) : (
        <ComposerPrimitive.Unstable_TriggerPopover.Action
          formatter={action.formatter ?? unstable_defaultDirectiveFormatter}
          onExecute={action.onExecute}
          removeOnExecute={action.removeOnExecute}
        />
      )}
      <TriggerPopoverCategories
        emptyLabel={emptyCategoriesLabel}
        fallbackIcon={fallbackIcon}
        iconMap={iconMap}
      />
      <TriggerPopoverItems
        backLabel={backLabel}
        emptyLabel={emptyItemsLabel}
        fallbackIcon={fallbackIcon}
        iconMap={iconMap}
      />
    </ComposerPrimitive.Unstable_TriggerPopover>
  )
}

function Composer(): React.JSX.Element {
  const mention = unstable_useMentionAdapter({ fallbackIcon: WrenchIcon })
  const slash = unstable_useSlashCommandAdapter({
    commands: slashCommands,
    fallbackIcon: SlashIcon,
    iconMap: slashIconMap
  })
  const modelSelector = useAppServerModelSelectorState()

  return (
    <ComposerPrimitive.Unstable_TriggerPopoverRoot>
      <ComposerPrimitive.Root className="aui-composer-root relative flex w-full flex-col">
        <div
          data-slot="aui_composer-shell"
          className="flex w-full flex-col gap-2 rounded-3xl border border-border/60 bg-background p-(--composer-padding) shadow-[0_4px_16px_-8px_rgba(0,0,0,0.08),0_1px_2px_rgba(0,0,0,0.04)] transition-[border-color,box-shadow] focus-within:border-border focus-within:shadow-[0_6px_24px_-8px_rgba(0,0,0,0.12),0_1px_2px_rgba(0,0,0,0.05)] dark:bg-muted/30"
        >
          <LexicalComposerInput
            className="aui-composer-input relative max-h-32 min-h-10 w-full resize-none overflow-y-auto bg-transparent px-2.5 py-1 text-base leading-6 outline-none [&_.aui-directive-chip]:inline-flex [&_.aui-directive-chip]:items-baseline [&_.aui-directive-chip]:gap-1 [&_.aui-directive-chip]:rounded-md [&_.aui-directive-chip]:bg-blue-100 [&_.aui-directive-chip]:px-1.5 [&_.aui-directive-chip]:py-0.5 [&_.aui-directive-chip]:text-[13px] [&_.aui-directive-chip]:leading-none [&_.aui-directive-chip]:font-medium [&_.aui-directive-chip]:text-blue-700 [&_.aui-directive-chip-icon]:self-center [&_.aui-lexical-input]:min-h-lh [&_.aui-lexical-input]:outline-none [&_.aui-lexical-placeholder]:pointer-events-none [&_.aui-lexical-placeholder]:absolute [&_.aui-lexical-placeholder]:inset-x-0 [&_.aui-lexical-placeholder]:top-0 [&_.aui-lexical-placeholder]:truncate [&_.aui-lexical-placeholder]:px-2.5 [&_.aui-lexical-placeholder]:py-1 [&_.aui-lexical-placeholder]:text-muted-foreground/80 dark:[&_.aui-directive-chip]:bg-blue-900/50 dark:[&_.aui-directive-chip]:text-blue-300"
            directiveChip={DirectiveChip}
            placeholder="输入消息（@ 提及工具，/ 输入命令）"
          />
          <div className="aui-composer-action-wrapper relative flex items-center justify-between">
            <div className="flex items-center gap-1">
              <ModelSelector
                models={modelSelector.models}
                value={modelSelector.value}
                onValueChange={modelSelector.onValueChange}
                variant="ghost"
                size="sm"
              />
            </div>
            <div className="flex items-center gap-1.5">
              <AuiIf condition={(state) => !state.thread.isRunning}>
                <ComposerPrimitive.Send asChild>
                  <IconButton
                    className="aui-composer-send size-7 rounded-full bg-primary text-primary-foreground hover:bg-primary/90 hover:text-primary-foreground"
                    label="发送消息"
                    title="发送消息"
                  >
                    <ArrowUpIcon className="size-4.5" />
                  </IconButton>
                </ComposerPrimitive.Send>
              </AuiIf>
              <AuiIf condition={(state) => state.thread.isRunning}>
                <ComposerPrimitive.Cancel asChild>
                  <IconButton
                    className="aui-composer-cancel size-7 rounded-full bg-primary text-primary-foreground hover:bg-primary/90 hover:text-primary-foreground"
                    label="停止生成"
                    title="停止生成"
                  >
                    <SquareIcon className="size-3.5 fill-current" />
                  </IconButton>
                </ComposerPrimitive.Cancel>
              </AuiIf>
            </div>
          </div>
        </div>
        <ComposerTriggerPopover char="@" {...mention} />
        <ComposerTriggerPopover char="/" emptyItemsLabel="没有匹配命令" {...slash} />
      </ComposerPrimitive.Root>
    </ComposerPrimitive.Unstable_TriggerPopoverRoot>
  )
}

const IconButton = forwardRef<HTMLButtonElement, IconButtonProps>(function IconButton(
  { children, className, label, title, type, ...buttonProps },
  ref
): React.JSX.Element {
  return (
    <button
      ref={ref}
      className={cn(
        'inline-grid size-8 shrink-0 place-items-center rounded-md text-muted-foreground transition-colors outline-none hover:bg-accent hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/40 disabled:cursor-not-allowed disabled:opacity-40',
        className
      )}
      type={type ?? 'button'}
      aria-label={label}
      title={title ?? label}
      {...buttonProps}
    >
      {children}
    </button>
  )
})

export default App
