import { CheckIcon, PlayIcon, ShieldCheckIcon, XIcon } from 'lucide-react'
import { useMemo, useState, type FormEvent } from 'react'

import { Button } from '@/components/ui/button'
import { runClientToolRequest } from '@/lib/clientTools'

import type {
  AppServerServerRequest,
  AppServerServerRequestResponse,
  AppServerToolUserInputResponse
} from '../../../../shared/appServerApi'

type ServerRequestPanelProps = {
  requests: readonly AppServerServerRequest[]
  onRespond: <Method extends AppServerServerRequest['method']>(
    request: AppServerServerRequest<Method>,
    response: AppServerServerRequestResponse<Method>
  ) => Promise<void>
  onReject: (request: AppServerServerRequest) => Promise<void>
}

type ActionState = 'accept' | 'acceptForSession' | 'allowTurn' | 'allowSession' | 'reject' | 'run'

export function ServerRequestPanel({
  requests,
  onRespond,
  onReject
}: ServerRequestPanelProps): React.JSX.Element | null {
  const request = requests[0]
  const [busyAction, setBusyAction] = useState<ActionState>()
  const [error, setError] = useState<string>()

  if (!request) return null

  const runAction = async (action: ActionState, callback: () => Promise<void>): Promise<void> => {
    setBusyAction(action)
    setError(undefined)
    try {
      await callback()
    } catch (caught) {
      setError(errorMessage(caught))
    } finally {
      setBusyAction(undefined)
    }
  }

  return (
    <section
      className="border-t border-border/70 bg-background px-4 py-3"
      data-slot="server-request-panel"
    >
      <div className="mx-auto flex w-full max-w-(--thread-max-width) flex-col gap-3">
        <div key={requestKey(request)}>
          {renderRequestBody(request, onRespond, onReject, runAction, busyAction)}
        </div>
        {error ? (
          <p className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm text-destructive">
            {error}
          </p>
        ) : null}
      </div>
    </section>
  )
}

function renderRequestBody(
  request: AppServerServerRequest,
  onRespond: ServerRequestPanelProps['onRespond'],
  onReject: ServerRequestPanelProps['onReject'],
  runAction: (action: ActionState, callback: () => Promise<void>) => Promise<void>,
  busyAction: ActionState | undefined
): React.JSX.Element {
  switch (request.method) {
    case 'item/fileChange/requestApproval':
      return (
        <RequestShell title="File change approval" method={request.method}>
          <Detail label="Reason" value={request.params.reason ?? 'No reason provided'} />
          {request.params.grantRoot ? (
            <Detail label="Grant root" value={request.params.grantRoot} />
          ) : null}
          <ActionRow>
            <Button
              disabled={Boolean(busyAction)}
              onClick={() =>
                void runAction('accept', () => onRespond(request, { decision: 'accept' }))
              }
              size="sm"
              type="button"
            >
              <CheckIcon className="size-4" />
              Accept file changes
            </Button>
            <Button
              disabled={Boolean(busyAction)}
              onClick={() =>
                void runAction('acceptForSession', () =>
                  onRespond(request, { decision: 'acceptForSession' })
                )
              }
              size="sm"
              type="button"
              variant="secondary"
            >
              <ShieldCheckIcon className="size-4" />
              Accept for session
            </Button>
            <RejectButton
              busy={busyAction === 'reject'}
              disabled={Boolean(busyAction)}
              onClick={() => void runAction('reject', () => onReject(request))}
            />
          </ActionRow>
        </RequestShell>
      )
    case 'item/permissions/requestApproval':
      return (
        <RequestShell title="Permission approval" method={request.method}>
          <Detail label="Reason" value={request.params.reason ?? 'No reason provided'} />
          <Detail label="Permissions" value={formatUnknown(request.params.permissions)} />
          <ActionRow>
            <Button
              disabled={Boolean(busyAction)}
              onClick={() =>
                void runAction('allowTurn', () =>
                  onRespond(request, {
                    decision: 'approve',
                    permissions: request.params.permissions,
                    scope: 'turn',
                    strictAutoReview: true
                  })
                )
              }
              size="sm"
              type="button"
            >
              <CheckIcon className="size-4" />
              Allow once
            </Button>
            <Button
              disabled={Boolean(busyAction)}
              onClick={() =>
                void runAction('allowSession', () =>
                  onRespond(request, {
                    decision: 'approve',
                    permissions: request.params.permissions,
                    scope: 'session',
                    strictAutoReview: true
                  })
                )
              }
              size="sm"
              type="button"
              variant="secondary"
            >
              <ShieldCheckIcon className="size-4" />
              Allow session
            </Button>
            <RejectButton
              busy={busyAction === 'reject'}
              disabled={Boolean(busyAction)}
              onClick={() => void runAction('reject', () => onReject(request))}
            />
          </ActionRow>
        </RequestShell>
      )
    case 'item/tool/requestUserInput':
      return (
        <ToolUserInputRequest
          busy={Boolean(busyAction)}
          onReject={() => void runAction('reject', () => onReject(request))}
          onSubmit={(response) => void runAction('accept', () => onRespond(request, response))}
          request={request}
        />
      )
    case 'item/tool/call':
      return (
        <RequestShell title="Client tool request" method={request.method}>
          <Detail
            label="Tool"
            value={
              [request.params.namespace, request.params.tool].filter(Boolean).join('.') ||
              request.params.tool
            }
          />
          <Detail label="Arguments" value={formatUnknown(request.params.arguments)} />
          <ActionRow>
            <Button
              disabled={Boolean(busyAction)}
              onClick={() =>
                void runAction('run', async () => {
                  const response = await runClientToolRequest(request)
                  await onRespond(request, response)
                })
              }
              size="sm"
              type="button"
            >
              <PlayIcon className="size-4" />
              Run client tool
            </Button>
            <RejectButton
              busy={busyAction === 'reject'}
              disabled={Boolean(busyAction)}
              onClick={() => void runAction('reject', () => onReject(request))}
            />
          </ActionRow>
        </RequestShell>
      )
    case 'item/commandExecution/requestApproval':
      return (
        <RequestShell title="Command execution approval" method={request.method}>
          <Detail label="Tool" value={request.params.toolName} />
          {request.params.command ? (
            <Detail label="Command" value={request.params.command} />
          ) : null}
          <Detail label="Description" value={request.params.description} />
          <Detail label="Parameters" value={formatUnknown(request.params.displayParameters)} />
          <ActionRow>
            <RejectButton
              busy={busyAction === 'reject'}
              disabled={Boolean(busyAction)}
              onClick={() => void runAction('reject', () => onReject(request))}
            />
          </ActionRow>
        </RequestShell>
      )
  }
}

function ToolUserInputRequest({
  busy,
  onReject,
  onSubmit,
  request
}: {
  busy: boolean
  onReject: () => void
  onSubmit: (response: AppServerToolUserInputResponse) => void
  request: AppServerServerRequest<'item/tool/requestUserInput'>
}): React.JSX.Element {
  const initialValues = useMemo(
    () => Object.fromEntries(request.params.questions.map((question) => [question.id, ''])),
    [request.params.questions]
  )
  const [values, setValues] = useState<Record<string, string>>(initialValues)

  const submit = (event: FormEvent<HTMLFormElement>): void => {
    event.preventDefault()
    onSubmit({
      answers: Object.fromEntries(
        request.params.questions.map((question) => [
          question.id,
          { answers: [values[question.id] ?? ''] }
        ])
      )
    })
  }

  return (
    <RequestShell title="User input requested" method={request.method}>
      <form className="flex flex-col gap-3" onSubmit={submit}>
        {request.params.questions.map((question) => (
          <label className="flex flex-col gap-1.5" key={question.id}>
            <span className="text-sm font-medium text-foreground">{question.header}</span>
            <span className="text-sm text-muted-foreground">{question.question}</span>
            <input
              className="h-9 rounded-md border border-input bg-background px-3 text-sm outline-none transition-shadow focus-visible:ring-[3px] focus-visible:ring-ring/50"
              disabled={busy}
              onChange={(event) =>
                setValues((current) => ({ ...current, [question.id]: event.target.value }))
              }
              type={question.isSecret ? 'password' : 'text'}
              value={values[question.id] ?? ''}
            />
          </label>
        ))}
        <ActionRow>
          <Button disabled={busy} size="sm" type="submit">
            <CheckIcon className="size-4" />
            Submit answers
          </Button>
          <RejectButton busy={busy} disabled={busy} onClick={onReject} />
        </ActionRow>
      </form>
    </RequestShell>
  )
}

function RequestShell({
  children,
  method,
  title
}: {
  children: React.ReactNode
  method: string
  title: string
}): React.JSX.Element {
  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div>
          <h2 className="text-sm font-semibold text-foreground">{title}</h2>
          <p className="text-xs text-muted-foreground">{method}</p>
        </div>
      </div>
      {children}
    </div>
  )
}

function Detail({ label, value }: { label: string; value: string }): React.JSX.Element {
  return (
    <dl className="grid gap-1 text-sm sm:grid-cols-[8rem_1fr]">
      <dt className="font-medium text-muted-foreground">{label}</dt>
      <dd className="min-w-0 whitespace-pre-wrap break-words text-foreground">{value}</dd>
    </dl>
  )
}

function ActionRow({ children }: { children: React.ReactNode }): React.JSX.Element {
  return <div className="flex flex-wrap items-center gap-2">{children}</div>
}

function RejectButton({
  busy,
  disabled,
  onClick
}: {
  busy: boolean
  disabled: boolean
  onClick: () => void
}): React.JSX.Element {
  return (
    <Button disabled={disabled} onClick={onClick} size="sm" type="button" variant="outline">
      <XIcon className="size-4" />
      {busy ? 'Rejecting' : 'Reject'}
    </Button>
  )
}

function formatUnknown(value: unknown): string {
  if (typeof value === 'string') return value
  try {
    return JSON.stringify(value, null, 2)
  } catch {
    return String(value)
  }
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

function requestKey(request: AppServerServerRequest): string {
  return `${request.hostId}:${request.requestId}`
}
