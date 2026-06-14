import {
  ActionBarPrimitive,
  AssistantRuntimeProvider,
  AuiIf,
  BranchPickerPrimitive,
  ComposerPrimitive,
  type AssistantState,
  MessagePrimitive,
  ThreadListItemPrimitive,
  ThreadListPrimitive,
  ThreadPrimitive,
  useAui,
  useAuiState
} from '@assistant-ui/react'
import {
  ActivityIcon,
  ArrowDownIcon,
  ArrowUpIcon,
  CheckIcon,
  ChevronLeftIcon,
  ChevronRightIcon,
  CopyIcon,
  PanelLeftIcon,
  PencilIcon,
  PlusIcon,
  RefreshCwIcon,
  SquareIcon
} from 'lucide-react'
import { forwardRef, useState, type ButtonHTMLAttributes, type ReactNode } from 'react'

import { ModelSelector } from './components/assistant-ui'
import { assistantModelOptions } from './lib/assistantMessages'
import { cn } from './lib/utils'
import { useDasclawAssistantRuntime } from './hooks/useDasclawAssistantRuntime'

type AppServerSidebarProps = {
  collapsed: boolean
}

type HeaderProps = {
  sidebarCollapsed: boolean
  onToggleSidebar: () => void
}

type IconButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  label: string
}

function App(): React.JSX.Element {
  const { runtime } = useDasclawAssistantRuntime()
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false)

  const toggleSidebar = (): void => {
    setSidebarCollapsed((collapsed) => !collapsed)
  }

  return (
    <AssistantRuntimeProvider runtime={runtime}>
      <main className="flex h-screen w-full bg-muted/30 text-foreground">
        <AppServerSidebar collapsed={sidebarCollapsed} />
        <section
          className={cn(
            'flex min-w-0 flex-1 flex-col overflow-hidden p-2 transition-[padding] duration-200',
            !sidebarCollapsed && 'md:pl-0'
          )}
        >
          <div className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-lg border border-border/50 bg-background shadow-[0_18px_60px_-48px_rgba(15,23,42,0.75)]">
            <Header sidebarCollapsed={sidebarCollapsed} onToggleSidebar={toggleSidebar} />
            <div className="min-h-0 flex-1 overflow-hidden">
              <ChatThread />
            </div>
          </div>
        </section>
      </main>
    </AssistantRuntimeProvider>
  )
}

function AppServerSidebar({ collapsed }: AppServerSidebarProps): React.JSX.Element {
  return (
    <aside
      className={cn(
        'hidden h-full shrink-0 flex-col overflow-hidden transition-all duration-200 md:flex',
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
            <ThreadList />
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

function ThreadList(): React.JSX.Element {
  return (
    <ThreadListPrimitive.Root className="flex flex-col gap-1">
      <ThreadListPrimitive.New asChild>
        <button
          className="inline-flex h-8 w-full items-center gap-2 rounded-md px-3 text-sm font-medium text-foreground transition-colors hover:bg-muted"
          type="button"
        >
          <PlusIcon className="size-4" />
          New thread
        </button>
      </ThreadListPrimitive.New>
      <ThreadListPrimitive.Items>{() => <ThreadListItem />}</ThreadListPrimitive.Items>
    </ThreadListPrimitive.Root>
  )
}

function ThreadListItem(): React.JSX.Element {
  return (
    <ThreadListItemPrimitive.Root className="flex min-h-8 items-center rounded-md transition-colors hover:bg-muted data-[active]:bg-muted">
      <ThreadListItemPrimitive.Trigger className="min-w-0 flex-1 truncate px-3 text-left text-sm font-medium text-foreground outline-none">
        <ThreadListItemPrimitive.Title fallback="New Chat" />
      </ThreadListItemPrimitive.Trigger>
    </ThreadListItemPrimitive.Root>
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
            {({ message }) => (message.role === 'user' ? <UserMessage /> : <AssistantMessage />)}
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
        今天想让 Dasclaw 做什么？
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
  return (
    <MessagePrimitive.Root
      data-slot="aui_assistant-message-root"
      data-role="assistant"
      className="relative mx-auto w-full max-w-(--thread-max-width) duration-150 animate-in fade-in slide-in-from-bottom-1"
    >
      <div
        data-slot="aui_assistant-message-content"
        className="wrap-break-word px-2 leading-relaxed text-foreground whitespace-pre-wrap"
      >
        <MessagePrimitive.Content />
        <MessagePrimitive.Error />
      </div>
      <div
        data-slot="aui_assistant-message-footer"
        className="ml-2 flex min-h-7.5 items-center pt-1.5 -mb-7.5"
      >
        <BranchPicker />
        <AssistantActionBar />
      </div>
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
      <div
        data-slot="aui_user-message-content"
        className="col-start-2 row-start-1 max-w-[min(85%,560px)] rounded-xl bg-muted px-4 py-2 text-foreground wrap-break-word whitespace-pre-wrap empty:hidden"
      >
        <MessagePrimitive.Content />
      </div>
      <div className="col-start-1 row-start-1 self-center justify-self-end pr-2">
        <ActionBarPrimitive.Root hideWhenRunning autohide="not-last">
          <ActionBarPrimitive.Edit asChild>
            <IconButton label="编辑" title="编辑">
              <PencilIcon className="size-4" />
            </IconButton>
          </ActionBarPrimitive.Edit>
        </ActionBarPrimitive.Root>
      </div>
      <BranchPicker className="col-span-full justify-end pr-1" />
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
      <ActionBarPrimitive.Reload asChild>
        <IconButton label="重新生成" title="重新生成">
          <RefreshCwIcon className="size-4" />
        </IconButton>
      </ActionBarPrimitive.Reload>
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

function Composer(): React.JSX.Element {
  return (
    <ComposerPrimitive.Root className="aui-composer-root relative flex w-full flex-col">
      <div
        data-slot="aui_composer-shell"
        className="flex w-full flex-col gap-2 rounded-3xl border border-border/60 bg-background p-(--composer-padding) shadow-[0_4px_16px_-8px_rgba(0,0,0,0.08),0_1px_2px_rgba(0,0,0,0.04)] transition-[border-color,box-shadow] focus-within:border-border focus-within:shadow-[0_6px_24px_-8px_rgba(0,0,0,0.12),0_1px_2px_rgba(0,0,0,0.05)]"
      >
        <ComposerPrimitive.Input
          className="aui-composer-input max-h-32 min-h-10 w-full resize-none bg-transparent px-2.5 py-1 text-base leading-6 outline-none placeholder:text-muted-foreground/80"
          placeholder="输入消息，按 Enter 发送"
          rows={1}
        />
        <div className="aui-composer-action-wrapper relative flex items-center justify-between">
          <div className="flex items-center gap-1">
            <ModelSelector
              models={assistantModelOptions}
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
    </ComposerPrimitive.Root>
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
