import {
  AlertTriangle,
  CheckCircle2,
  Circle,
  Loader2,
  RefreshCw,
  Search,
  Upload,
  XCircle,
} from 'lucide-react'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { Textarea } from '@/components/ui/textarea'
import { TablePagination } from '@/components/ui/table-pagination'
import { ToggleSwitch } from '@/components/ui/toggle-switch'
import { api } from '@/lib/api'
import { toast } from 'sonner'

type ReviewStatus = 'scanning' | 'pending' | 'approved' | 'rejected' | 'scan_failed' | 'yanked'
type ReviewFilter = 'all' | 'needs_scan' | 'pending' | 'approved' | 'rejected' | 'yanked'
type TabKey = 'skills' | 'plugins'
type UploadState = 'pick' | 'validation_failed' | 'scanning' | 'completed'

const UPLOAD_SLOW_SPIN_CLASS = 'animate-spin [animation-duration:1.8s]'

interface SkillItem {
  id: string
  name: string
  description?: string
  version: string
  author?: string
  enabled: boolean
  source: 'builtin' | 'admin_upload'
  review_status: ReviewStatus
  is_builtin: boolean
  invoke_count: number
  review_note?: string
  reviewed_by?: string
  reviewed_at?: string
  updated_at: string
}

interface PluginItem {
  id: string
  name: string
  description?: string
  version: string
  author?: string
  enabled: boolean
  source: 'builtin' | 'admin_upload'
  review_status: ReviewStatus
  plugin_type: 'http' | 'stdio' | 'wasm'
  is_builtin: boolean
  invoke_count: number
  requires_sandbox: boolean
  review_note?: string
  reviewed_by?: string
  reviewed_at?: string
  updated_at: string
}

interface ReviewTarget {
  id: string
  name: string
  type: 'skill' | 'plugin'
}

interface SkillUploadResponse {
  id: string
  name: string
  review_status: ReviewStatus
  message?: string
  scan_result?: Record<string, unknown> | null
  scan_runtime?: Record<string, unknown> | null
}

interface UploadScanRuntime {
  use_llm: boolean
  llm_provider: string
  llm_model?: string
  llm_base_url?: string
  llm_api_version?: string
  llm_model_config_id?: string
  llm_api_key_configured: boolean
}

interface UploadModelConfigOption {
  id: string
  modelId: string
  displayName: string
  provider: string
  enabled: boolean
}

interface ScanFinding {
  rule_id?: string
  severity?: string
  title?: string
  location?: string
  snippet?: string
  recommendation?: string
}

interface ScanResultPayload {
  scanner_type: string
  verdict: string
  is_safe: boolean
  max_severity?: string
  findings_count: number
  findings: ScanFinding[]
  scan_duration_ms?: number
  scanned_at?: string
  created_at: string
}

const REVIEW_FILTER_OPTIONS: ReviewFilter[] = ['all', 'needs_scan', 'pending', 'approved', 'rejected', 'yanked']

function scanResultsEndpoint(target: ReviewTarget): string {
  return target.type === 'skill'
    ? `/skills/${target.id}/scan-results`
    : `/plugins/${target.id}/scan-results`
}

function tableActionButtonClass(tone: 'default' | 'danger' = 'default'): string {
  const base = 'inline-flex h-7 items-center gap-1.5 border border-[#E8E8E8] bg-white px-3 py-1.5 font-mono text-[9px] font-semibold leading-none'
  return tone === 'danger' ? `${base} text-[#CF1322]` : `${base} text-[#1A1A1A]`
}

function extractApiMessage(error: unknown, fallback: string): string {
  if (typeof error === 'object' && error !== null) {
    const maybeResponse = (error as {
      response?: {
        data?: {
          message?: unknown
          details?: unknown
          error?: unknown
        }
      }
    }).response
    const data = maybeResponse?.data

    const candidates = [data?.details, data?.message, data?.error]
    for (const candidate of candidates) {
      if (typeof candidate === 'string' && candidate.trim().length > 0) {
        return candidate
      }
      if (Array.isArray(candidate)) {
        const joined = candidate
          .filter((item): item is string => typeof item === 'string' && item.trim().length > 0)
          .join(' | ')
        if (joined.length > 0) {
          return joined
        }
      }
    }
  }

  if (error instanceof Error && error.message.trim().length > 0) {
    return error.message
  }

  return fallback
}

function parseValidationErrors(message: string): string[] {
  const body = message.replace(/\s+/g, ' ').trim()
  if (!body) {
    return []
  }

  const withPipes = body.replace(/[;；]/g, '|').replace(/\s*\|\s*/g, '|')
  const parts = withPipes
    .split('|')
    .map((item) => item.trim())
    .filter(Boolean)

  if (parts.length > 0) {
    return parts
  }

  return [body]
}

function normalizeUploadModelConfigs(input: unknown): UploadModelConfigOption[] {
  if (!Array.isArray(input)) {
    return []
  }

  return input.reduce<UploadModelConfigOption[]>((acc, item) => {
    if (typeof item !== 'object' || item === null) {
      return acc
    }

    const row = item as Record<string, unknown>
    const id = typeof row.id === 'string' ? row.id : ''
    const modelId = typeof row.model_id === 'string' ? row.model_id : ''
    const displayName = typeof row.display_name === 'string' ? row.display_name : modelId
    const provider = typeof row.provider === 'string' ? row.provider : 'unknown'
    const enabled = row.enabled === true

    if (!id || !modelId) {
      return acc
    }

    acc.push({ id, modelId, displayName, provider, enabled })
    return acc
  }, [])
}

function mapUploadScanRuntime(input: unknown): UploadScanRuntime | null {
  if (!input || typeof input !== 'object') {
    return null
  }

  const row = input as Record<string, unknown>
  const hasAnyLlmField =
    typeof row.llm_provider === 'string'
    || typeof row.llm_model === 'string'
    || typeof row.llm_base_url === 'string'
    || typeof row.llm_api_version === 'string'
    || row.use_llm === true
    || row.use_llm === false

  if (!hasAnyLlmField) {
    return null
  }

  return {
    use_llm: row.use_llm === true,
    llm_provider: typeof row.llm_provider === 'string' ? row.llm_provider : 'unknown',
    llm_model: typeof row.llm_model === 'string' ? row.llm_model : undefined,
    llm_base_url: typeof row.llm_base_url === 'string' ? row.llm_base_url : undefined,
    llm_api_version: typeof row.llm_api_version === 'string' ? row.llm_api_version : undefined,
    llm_model_config_id: typeof row.llm_model_config_id === 'string' ? row.llm_model_config_id : undefined,
    llm_api_key_configured: row.llm_api_key_configured === true,
  }
}

function firstStringField(row: Record<string, unknown>, keys: string[]): string | undefined {
  for (const key of keys) {
    const value = row[key]
    if (typeof value === 'string') {
      return value
    }
  }
  return undefined
}

function normalizeFindings(input: unknown): ScanFinding[] {
  if (!Array.isArray(input)) {
    return []
  }

  const normalized = input.reduce<ScanFinding[]>((acc, item) => {
    if (typeof item !== 'object' || item === null) {
      return acc
    }

    const row = item as Record<string, unknown>
    const locationPath = firstStringField(row, ['location', 'file', 'file_path'])
    const ruleId = firstStringField(row, ['rule_id', 'id'])
    const severity = firstStringField(row, ['severity', 'level'])
    const title = firstStringField(row, ['title', 'message'])
    const snippet = firstStringField(row, ['code_snippet', 'snippet'])
    const recommendation = firstStringField(row, ['recommendation', 'suggestion'])
    const line = typeof row.line === 'number' ? `:${row.line}` : ''

    acc.push({
      rule_id: ruleId,
      severity: severity?.toUpperCase(),
      title,
      location: locationPath ? `${locationPath}${line}` : undefined,
      snippet,
      recommendation,
    })

    return acc
  }, [])

  return dedupeFindings(normalized)
}

function scanStepMeta(state: UploadState): { status: 'running' | 'warn' | 'idle'; subtitle: string } {
  if (state === 'scanning') {
    return { status: 'running', subtitle: '等待扫描步骤完成' }
  }
  if (state === 'completed') {
    return { status: 'warn', subtitle: '扫描已完成' }
  }
  return { status: 'idle', subtitle: '等待前置步骤完成' }
}

function submitQueueStepMeta(
  state: UploadState,
  reviewStatus: ReviewStatus | undefined,
): { status: 'idle' | 'warn' | 'done'; subtitle: string } {
  if (state !== 'completed') {
    return { status: 'idle', subtitle: '等待前置步骤完成' }
  }
  if (reviewStatus === 'scan_failed') {
    return { status: 'warn', subtitle: '扫描失败，未进入审核队列' }
  }
  return { status: 'done', subtitle: '状态：pending，等待管理员审核' }
}

function normalizeFindingSnippet(snippet: string | undefined): string {
  if (!snippet) {
    return ''
  }
  return snippet
    .toLowerCase()
    .replace(/[^\p{L}\p{N}]+/gu, '')
}

function isPromptRuleId(ruleId: string | undefined): boolean {
  if (!ruleId) {
    return false
  }
  const normalized = ruleId.toUpperCase()
  return normalized.includes('PROMPT') || normalized.includes('INJECTION')
}

function dedupeFindings(findings: ScanFinding[]): ScanFinding[] {
  const promptSnippetWithScannerRule = new Set<string>()

  findings.forEach((finding) => {
    const snippetKey = normalizeFindingSnippet(finding.snippet)
    if (!snippetKey || !isPromptRuleId(finding.rule_id)) {
      return
    }

    if (finding.rule_id !== 'PROMPT_INJECTION_INSTRUCTION_OVERRIDE') {
      promptSnippetWithScannerRule.add(snippetKey)
    }
  })

  const seenExact = new Set<string>()
  return findings.filter((finding) => {
    const snippetKey = normalizeFindingSnippet(finding.snippet)
    if (
      finding.rule_id === 'PROMPT_INJECTION_INSTRUCTION_OVERRIDE'
      && snippetKey
      && promptSnippetWithScannerRule.has(snippetKey)
    ) {
      return false
    }

    const exactKey = [
      finding.rule_id ?? '',
      finding.severity ?? '',
      finding.title ?? '',
      finding.location ?? '',
      snippetKey,
    ].join('|')

    if (seenExact.has(exactKey)) {
      return false
    }
    seenExact.add(exactKey)
    return true
  })
}

function isSupportedSkillPackage(fileName: string): boolean {
  const lower = fileName.toLowerCase()
  return lower.endsWith('.zip') || lower.endsWith('.tar.gz') || lower.endsWith('.tgz')
}

function reviewFilterLabel(filter: ReviewFilter): string {
  const labelMap: Record<ReviewFilter, string> = {
    all: '全部',
    needs_scan: '待扫描',
    pending: '待审核',
    approved: '已通过',
    rejected: '已拒绝',
    yanked: '已下架',
  }
  return labelMap[filter]
}

function matchesReviewFilter(status: ReviewStatus, filter: ReviewFilter): boolean {
  if (filter === 'all') {
    return true
  }
  if (filter === 'needs_scan') {
    return status === 'scanning' || status === 'scan_failed'
  }
  return status === filter
}

function countByFilter<T extends { review_status: ReviewStatus }>(items: T[], filter: ReviewFilter): number {
  return items.filter((item) => matchesReviewFilter(item.review_status, filter)).length
}

function reviewHistoryLabel(item: { reviewed_by?: string; reviewed_at?: string }): string {
  if (!item.reviewed_at) {
    return '—'
  }

  const reviewer = item.reviewed_by ? item.reviewed_by.slice(0, 8) : 'system'
  const reviewedAt = new Date(item.reviewed_at)
  if (Number.isNaN(reviewedAt.getTime())) {
    return `${reviewer} / -`
  }

  return `${reviewer} / ${reviewedAt.toLocaleString('zh-CN', { hour12: false })}`
}

function reviewNoteText(note?: string): string {
  if (!note) {
    return ''
  }
  return note.trim()
}

function sourceBadgeStyle(source: string): { background: string; color: string } {
  if (source === 'builtin') {
    return { background: '#F0F0F0', color: '#666666' }
  }
  return { background: '#E8F5E9', color: '#0A6B3A' }
}

function reviewBadgeMeta(status: ReviewStatus): { label: string; bg: string; color: string } {
  const map: Record<ReviewStatus, { label: string; bg: string; color: string }> = {
    scanning: { label: '扫描中', bg: '#EFF6FF', color: '#1D4ED8' },
    pending: { label: '待审核', bg: '#FEF3C7', color: '#B45309' },
    approved: { label: '已通过', bg: '#E8F5E9', color: '#0A6B3A' },
    rejected: { label: '已拒绝', bg: '#FEE2E2', color: '#DC2626' },
    scan_failed: { label: '扫描失败', bg: '#FFF7ED', color: '#EA580C' },
    yanked: { label: '已下架', bg: '#F3F4F6', color: '#6B7280' },
  }
  return map[status]
}

function verdictBadgeMeta(verdict: string): { label: string; bg: string; color: string } {
  const key = verdict.toUpperCase()
  const map: Record<string, { label: string; bg: string; color: string }> = {
    SAFE: { label: 'SAFE', bg: '#E8F5E9', color: '#0A6B3A' },
    SUSPICIOUS: { label: 'SUSPICIOUS', bg: '#FEF3C7', color: '#B45309' },
    DANGEROUS: { label: 'DANGEROUS', bg: '#FFF7ED', color: '#EA580C' },
    BLOCKED: { label: 'BLOCKED', bg: '#FEE2E2', color: '#DC2626' },
  }
  return map[key] ?? { label: key || 'UNKNOWN', bg: '#F3F4F6', color: '#6B7280' }
}

function severityBadgeMeta(severity: string): { label: string; bg: string; color: string } {
  const key = severity.toUpperCase()
  const map: Record<string, { label: string; bg: string; color: string }> = {
    LOW: { label: 'LOW', bg: '#E8F5E9', color: '#0A6B3A' },
    MEDIUM: { label: 'MEDIUM', bg: '#FEF3C7', color: '#B45309' },
    HIGH: { label: 'HIGH', bg: '#FFF7ED', color: '#EA580C' },
    CRITICAL: { label: 'CRITICAL', bg: '#FEE2E2', color: '#DC2626' },
  }
  return map[key] ?? { label: key, bg: '#F3F4F6', color: '#6B7280' }
}

function SourceBadge({ source }: { source: string }) {
  const style = sourceBadgeStyle(source)
  return (
    <span
      className="inline-flex items-center px-1.5 py-0.5 font-mono text-[9px] font-semibold"
      style={{ background: style.background, color: style.color, border: '1px solid #E8E8E8' }}
    >
      {source === 'builtin' ? '内置' : '上传'}
    </span>
  )
}

function ReviewBadge({ status }: { status: ReviewStatus }) {
  const meta = reviewBadgeMeta(status)
  return (
    <span
      className="inline-flex items-center px-1.5 py-0.5 font-mono text-[9px] font-semibold"
      style={{ background: meta.bg, color: meta.color, border: '1px solid #E8E8E8' }}
    >
      {meta.label}
    </span>
  )
}

function VerdictBadge({ verdict }: { verdict: string }) {
  const meta = verdictBadgeMeta(verdict)
  return (
    <span className="inline-flex items-center px-1.5 py-0.5 font-mono text-[9px] font-semibold" style={{ background: meta.bg, color: meta.color }}>
      {meta.label}
    </span>
  )
}

function SeverityBadge({ severity }: { severity: string }) {
  const meta = severityBadgeMeta(severity)
  return (
    <span className="inline-flex items-center px-1.5 py-0.5 font-mono text-[9px] font-semibold" style={{ background: meta.bg, color: meta.color }}>
      {meta.label}
    </span>
  )
}

function ReviewHistoryCell({ item }: { item: { reviewed_by?: string; reviewed_at?: string; review_note?: string } }) {
  const summary = reviewHistoryLabel(item)
  const note = reviewNoteText(item.review_note)

  if (!note) {
    return <span className="font-mono text-[10px] text-[#999999]">{summary}</span>
  }

  return (
    <details className="group">
      <summary className="cursor-pointer list-none font-mono text-[10px] text-[#999999]">
        <span>{summary}</span>
        <span className="ml-1 text-[#0A6B3A]">备注</span>
      </summary>
      <p className="mt-1 whitespace-pre-wrap break-words font-mono text-[10px] text-[#666666]">{note}</p>
    </details>
  )
}

function StepRow({
  index,
  title,
  subtitle,
  status,
}: {
  index: number
  title: string
  subtitle: string
  status: 'done' | 'error' | 'running' | 'warn' | 'idle'
}) {
  if (status === 'done') {
    return (
      <div className="space-y-1">
        <div className="flex items-center gap-2 font-mono text-[10px] font-semibold text-[#0A6B3A]">
          <CheckCircle2 className="h-3.5 w-3.5 text-[#0A6B3A]" />
          Step {index} - {title}
        </div>
        <p className="pl-5 font-mono text-[9px] text-[#999999]">{subtitle}</p>
      </div>
    )
  }

  if (status === 'error') {
    return (
      <div className="space-y-1">
        <div className="flex items-center gap-2 font-mono text-[10px] font-semibold text-[#CF1322]">
          <XCircle className="h-3.5 w-3.5" />
          Step {index} - {title}
        </div>
        <p className="pl-5 font-mono text-[9px] text-[#CF1322]">{subtitle}</p>
      </div>
    )
  }

  if (status === 'running') {
    return (
      <div className="space-y-1">
        <div className="flex items-center gap-2 font-mono text-[10px] font-semibold text-[#1677FF]">
          <Loader2 className={`h-3.5 w-3.5 ${UPLOAD_SLOW_SPIN_CLASS}`} />
          Step {index} - {title}...
        </div>
        <p className="pl-5 font-mono text-[9px] text-[#999999]">{subtitle}</p>
      </div>
    )
  }

  if (status === 'warn') {
    return (
      <div className="space-y-1">
        <div className="flex items-center gap-2 font-mono text-[10px] font-semibold text-[#D48700]">
          <AlertTriangle className="h-3.5 w-3.5" />
          Step {index} - {title}完成
        </div>
        <p className="pl-5 font-mono text-[9px] text-[#999999]">{subtitle}</p>
      </div>
    )
  }

  return (
    <div className="space-y-1">
      <div className="flex items-center gap-2 font-mono text-[10px] font-semibold text-[#999999]">
        <Circle className="h-3.5 w-3.5" />
        Step {index} - {title}
      </div>
      <p className="pl-5 font-mono text-[9px] text-[#999999]">{subtitle}</p>
    </div>
  )
}

interface UploadSkillModalProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  onSuccess: () => void
  onReviewRequest: (target: ReviewTarget) => void
}

interface UploadPickPanelProps {
  file: File | null
  fileRef: { current: HTMLInputElement | null }
  modelOptions: UploadModelConfigOption[]
  selectedModelConfigId: string
  modelLoading: boolean
  modelError: string
  onPickFile: (event: React.ChangeEvent<HTMLInputElement>) => void
  onModelConfigChange: (modelConfigId: string) => void
}

interface UploadStatusPanelProps {
  state: UploadState
  file: File | null
  errors: string[]
  scanNotes: string[]
  scanProgress: number
  serverResponse: SkillUploadResponse | null
  uploadScanResult: ScanResultPayload | null
}

interface UploadModalFooterProps {
  state: UploadState
  loading: boolean
  canSubmit: boolean
  disabledTooltip: string
  canReviewNow: boolean
  onCancel: () => void
  onSubmit: () => void
  onReviewNow: () => void
}

function formatUploadFileLabel(file: File | null): string {
  if (!file) {
    return '尚未选择文件'
  }
  return `${file.name} (${(file.size / 1024 / 1024).toFixed(1)}MB)`
}

interface UploadLlmConfigPanelProps {
  modelOptions: UploadModelConfigOption[]
  selectedModelConfigId: string
  modelLoading: boolean
  modelError: string
  onModelConfigChange: (modelConfigId: string) => void
}

function UploadLlmConfigPanel({
  modelOptions,
  selectedModelConfigId,
  modelLoading,
  modelError,
  onModelConfigChange,
}: UploadLlmConfigPanelProps) {
  const hasOptions = modelOptions.length > 0

  return (
    <div className="space-y-3 border border-[#E8E8E8] bg-[#FAFAFA] px-4 py-3">
      <div>
        <p className="font-mono text-[10px] font-semibold text-[#1A1A1A]">启用 LLM 深度扫描</p>
        <p className="font-mono text-[9px] text-[#666666]">请先选择扫描模型，未选择时上传框将禁用</p>
      </div>

      <div className="grid gap-2">
        {modelError && <p className="font-mono text-[9px] text-[#CF1322]">{modelError}</p>}
        <label className="grid gap-1 font-mono text-[9px] text-[#666666]">
          扫描模型配置
          <select
            value={selectedModelConfigId}
            disabled={modelLoading || !hasOptions}
            onChange={(event) => onModelConfigChange(event.target.value)}
            className="h-8 border border-[#D9D9D9] bg-white px-2 font-mono text-[10px] text-[#1A1A1A] disabled:bg-[#F5F5F5]"
          >
            {!hasOptions && <option value="">暂无可用模型配置</option>}
            {modelOptions.map((option) => (
              <option key={option.id} value={option.id}>
                {option.displayName} ({option.provider} / {option.modelId})
              </option>
            ))}
          </select>
        </label>
        <p className="font-mono text-[9px] text-[#666666]">将自动使用所选模型配置中的 provider 和 API Key</p>
      </div>
    </div>
  )
}

function UploadPickPanel({
  file,
  fileRef,
  modelOptions,
  selectedModelConfigId,
  modelLoading,
  modelError,
  onPickFile,
  onModelConfigChange,
}: UploadPickPanelProps) {
  const canPickFile = selectedModelConfigId.length > 0

  return (
    <>
      <UploadLlmConfigPanel
        modelOptions={modelOptions}
        selectedModelConfigId={selectedModelConfigId}
        modelLoading={modelLoading}
        modelError={modelError}
        onModelConfigChange={onModelConfigChange}
      />

      <input
        ref={fileRef}
        type="file"
        accept=".zip,.tar.gz,.tgz"
        className="hidden"
        disabled={!canPickFile}
        onChange={onPickFile}
      />
      <button
        type="button"
        disabled={!canPickFile}
        onClick={() => {
          if (!canPickFile) {
            return
          }
          fileRef.current?.click()
        }}
        className="flex h-40 w-full items-center justify-center border border-dashed border-[#E8E8E8] bg-white px-4 font-mono text-[10px] font-medium text-[#999999] transition hover:border-[#0A6B3A] hover:text-[#0A6B3A] disabled:cursor-not-allowed disabled:border-[#E8E8E8] disabled:bg-[#F5F5F5] disabled:text-[#BBBBBB]"
      >
        {canPickFile ? '拖拽文件到此处，或点击选择 支持 .zip / .tar.gz 格式' : '请先选择 LLM 模型配置，随后可上传文件'}
      </button>
      <p className="font-mono text-[10px] font-medium text-[#1A1A1A]">待上传：{formatUploadFileLabel(file)}</p>
    </>
  )
}

function UploadStatusPanel({
  state,
  file,
  errors,
  scanNotes,
  scanProgress,
  serverResponse,
  uploadScanResult,
}: UploadStatusPanelProps) {
  const uploadScanRuntime = mapUploadScanRuntime(serverResponse?.scan_runtime)
  const scanStep = scanStepMeta(state)
  const submitQueueStep = submitQueueStepMeta(state, serverResponse?.review_status)

  return (
    <div className="space-y-3">
      <StepRow index={1} title="文件解析" status="done" subtitle={file?.name ?? '文件已读取'} />
      <StepRow
        index={2}
        title="SKILL.md 格式校验"
        status={state === 'validation_failed' ? 'error' : 'done'}
        subtitle={state === 'validation_failed' ? '校验失败，已终止' : '服务端校验通过'}
      />

      {state === 'validation_failed' && errors.length > 0 && (
        <div className="border border-[#CF132240] bg-[#CF132208] px-4 py-3 text-[#CF1322]">
          <ul className="space-y-1 font-mono text-[10px]">
            {errors.map((item) => (
              <li key={item}>• {item}</li>
            ))}
          </ul>
        </div>
      )}

      <StepRow
        index={3}
        title="安全扫描"
        status={scanStep.status}
        subtitle={scanStep.subtitle}
      />

      {state === 'scanning' && (
        <div className="border border-[#1677FF40] bg-[#1677FF08] px-4 py-3">
          <p className="mb-2 font-mono text-[9px] text-[#5B8FF9]">当前扫描内容：</p>
          <ul className="space-y-1 font-mono text-[10px] text-[#1677FF]">
            {scanNotes.map((item, index) => (
              <li key={item} className="flex items-center gap-2">
                {index < scanProgress ? (
                  <CheckCircle2 className="h-3.5 w-3.5 text-[#0A6B3A]" />
                ) : index === scanProgress ? (
                  <Loader2 className={`h-3.5 w-3.5 ${UPLOAD_SLOW_SPIN_CLASS}`} />
                ) : (
                  <Circle className="h-3.5 w-3.5 text-[#91C3FF]" />
                )}
                {item}
              </li>
            ))}
          </ul>
        </div>
      )}

      {state === 'completed' && serverResponse && (
        <div className="border border-[#D4870040] bg-[#D4870008] px-4 py-3 text-[#D48700]">
          <p className="font-mono text-[10px] font-semibold">
            {serverResponse.review_status === 'scan_failed'
              ? '扫描未通过，请修复后重扫'
              : '扫描通过，已进入待审核队列'}
          </p>
          {uploadScanResult && (
            <div className="mt-2 space-y-1 border-t border-[#D4870040] pt-2">
              <p className="font-mono text-[10px]">
                扫描器：{uploadScanResult.scanner_type} | 风险结论：{uploadScanResult.verdict}
              </p>
              <p className="font-mono text-[10px]">
                发现项：{uploadScanResult.findings_count}
                {typeof uploadScanResult.scan_duration_ms === 'number' && ` | 耗时：${uploadScanResult.scan_duration_ms}ms`}
              </p>
              {uploadScanResult.findings.length > 0 && (
                <ul className="space-y-1 font-mono text-[10px]">
                  {uploadScanResult.findings.slice(0, 3).map((finding, index) => (
                    <li key={`${finding.rule_id ?? 'finding'}-${index}`} className="space-y-1">
                      <p>
                        • {(finding.severity ?? 'UNKNOWN').toUpperCase()} - {finding.title ?? finding.rule_id ?? `发现问题 #${index + 1}`}
                      </p>
                      {finding.snippet && (
                        <pre className="bg-[#FFF7E6] px-2 py-1 text-[9px] text-[#8A6A00] whitespace-pre-wrap">
                          {finding.snippet}
                        </pre>
                      )}
                    </li>
                  ))}
                </ul>
              )}
            </div>
          )}

          {uploadScanRuntime && (
            <div className="mt-2 space-y-1 border-t border-[#D4870040] pt-2">
              <p className="font-mono text-[10px] font-semibold">本次实际 LLM 参数</p>
              <p className="font-mono text-[10px]">
                use_llm：{uploadScanRuntime.use_llm ? 'true' : 'false'} | provider：{uploadScanRuntime.llm_provider}
              </p>
              {uploadScanRuntime.llm_model && (
                <p className="font-mono text-[10px]">model：{uploadScanRuntime.llm_model}</p>
              )}
              {uploadScanRuntime.llm_base_url && (
                <p className="font-mono text-[10px] break-all">base_url：{uploadScanRuntime.llm_base_url}</p>
              )}
              {uploadScanRuntime.llm_api_version && (
                <p className="font-mono text-[10px]">api_version：{uploadScanRuntime.llm_api_version}</p>
              )}
              {uploadScanRuntime.llm_model_config_id && (
                <p className="font-mono text-[10px] break-all">model_config_id：{uploadScanRuntime.llm_model_config_id}</p>
              )}
              <p className="font-mono text-[10px]">
                api_key：{uploadScanRuntime.llm_api_key_configured ? '已配置（隐藏）' : '未配置'}
              </p>
            </div>
          )}

          {!uploadScanResult && (
            <p className="mt-2 font-mono text-[10px] text-[#8A6A00]">
              扫描结果尚未落库，可在“审核”弹窗中查看完整扫描报告
            </p>
          )}
        </div>
      )}

      <StepRow
        index={4}
        title="提交审核队列"
        status={submitQueueStep.status}
        subtitle={submitQueueStep.subtitle}
      />
    </div>
  )
}

function UploadModalFooter({
  state,
  loading,
  canSubmit,
  disabledTooltip,
  canReviewNow,
  onCancel,
  onSubmit,
  onReviewNow,
}: UploadModalFooterProps) {
  const submitDisabled = loading || !canSubmit

  if (state === 'completed') {
    return (
      <DialogFooter inset={false} className="flex-wrap items-center justify-end gap-3 border-[#E8E8E8] bg-white px-6 py-4" showCloseButton={false}>
        <button type="button" onClick={onCancel} className="border border-[#E8E8E8] bg-white px-5 py-2.5 font-mono text-[10px] font-semibold text-[#1A1A1A]">
          稍后处理
        </button>
        {canReviewNow ? (
          <button
            type="button"
            onClick={onReviewNow}
            className="flex items-center gap-2 bg-[#0A6B3A] px-5 py-2.5 font-mono text-[10px] font-semibold text-white"
          >
            <CheckCircle2 className="h-3 w-3" />
            审核
          </button>
        ) : (
          <button
            type="button"
            onClick={onCancel}
            className="flex items-center gap-2 bg-[#0A6B3A] px-5 py-2.5 font-mono text-[10px] font-semibold text-white"
          >
            <CheckCircle2 className="h-3 w-3" />
            确认关闭
          </button>
        )}
      </DialogFooter>
    )
  }

  return (
    <DialogFooter inset={false} className="flex-wrap items-center justify-end gap-3 border-[#E8E8E8] bg-white px-6 py-4" showCloseButton={false}>
      <button type="button" onClick={onCancel} className="border border-[#E8E8E8] bg-white px-5 py-2.5 font-mono text-[10px] font-semibold text-[#1A1A1A]">
        取消
      </button>
      <Button
        type="button"
        onClick={onSubmit}
        disabled={submitDisabled}
        disabledTooltip={disabledTooltip}
        className="h-auto rounded-none bg-[#0A6B3A] px-5 py-2.5 font-mono text-[10px] font-semibold text-white"
      >
        {loading ? <Loader2 className={`h-3 w-3 ${UPLOAD_SLOW_SPIN_CLASS}`} /> : <CheckCircle2 className="h-3 w-3" />}
        开始上传
      </Button>
    </DialogFooter>
  )
}

function useUploadSkillModalState(open: boolean, onSuccess: () => void) {
  const [file, setFile] = useState<File | null>(null)
  const [modelOptions, setModelOptions] = useState<UploadModelConfigOption[]>([])
  const [selectedModelConfigId, setSelectedModelConfigId] = useState('')
  const [modelLoading, setModelLoading] = useState(false)
  const [modelError, setModelError] = useState('')
  const [state, setState] = useState<UploadState>('pick')
  const [errors, setErrors] = useState<string[]>([])
  const [scanNotes, setScanNotes] = useState<string[]>([])
  const [scanProgress, setScanProgress] = useState(0)
  const [loading, setLoading] = useState(false)
  const [serverResponse, setServerResponse] = useState<SkillUploadResponse | null>(null)
  const [uploadScanResult, setUploadScanResult] = useState<ScanResultPayload | null>(null)
  const fileRef = useRef<HTMLInputElement | null>(null)

  // Buffer API result so scan animation can finish before state transition
  const pendingResult = useRef<{
    response?: SkillUploadResponse
    errors?: string[]
    scanResult?: ScanResultPayload | null
  } | null>(null)
  const scanDone = useRef(false)

  useEffect(() => {
    if (open) {
      setFile(null)
      setModelError('')
      setState('pick')
      setErrors([])
      setScanNotes([])
      setScanProgress(0)
      setLoading(false)
      setServerResponse(null)
      setUploadScanResult(null)
      pendingResult.current = null
      scanDone.current = false
    }
  }, [open])

  useEffect(() => {
    if (!open) {
      return
    }

    let cancelled = false
    const loadModelOptions = async () => {
      setModelLoading(true)
      setModelError('')
      try {
        const res = await api.get('/model-configs')
        if (cancelled) {
          return
        }
        const normalized = normalizeUploadModelConfigs(res.data)
        const enabledOnly = normalized.filter((item) => item.enabled)
        setModelOptions(enabledOnly)
        setSelectedModelConfigId((prev) => {
          if (prev && enabledOnly.some((item) => item.id === prev)) {
            return prev
          }
          return enabledOnly[0]?.id ?? ''
        })
        if (enabledOnly.length === 0) {
          setModelError('暂无启用中的模型配置，请先到“模型配置”页面启用')
        }
      } catch (error: unknown) {
        if (!cancelled) {
          setModelOptions([])
          setSelectedModelConfigId('')
          setModelError(extractApiMessage(error, '加载模型配置失败'))
        }
      } finally {
        if (!cancelled) {
          setModelLoading(false)
        }
      }
    }

    void loadModelOptions()
    return () => {
      cancelled = true
    }
  }, [open])

  // Flush buffered API result once scan animation completes
  const flushPending = useCallback(() => {
    const buf = pendingResult.current
    if (!buf) return
    pendingResult.current = null
    setLoading(false)
    if (buf.errors) {
      setErrors(buf.errors)
      setState('validation_failed')
    } else if (buf.response) {
      setServerResponse(buf.response)
      setUploadScanResult(buf.scanResult ?? null)
      setState('completed')
      if (buf.response.review_status === 'pending') {
        toast.success('上传成功，已进入待审核队列')
      }
      onSuccess()
    }
  }, [onSuccess])

  useEffect(() => {
    if (state !== 'scanning' || scanNotes.length === 0) {
      return
    }

    scanDone.current = false
    setScanProgress(0)
    const timer = window.setInterval(() => {
      setScanProgress((prev) => {
        const next = prev + 1
        if (next >= scanNotes.length) {
          window.clearInterval(timer)
          scanDone.current = true
          // If API already returned, flush on next tick
          if (pendingResult.current) {
            window.setTimeout(flushPending, 400)
          }
          return prev
        }
        return next
      })
    }, 900)

    return () => window.clearInterval(timer)
  }, [state, scanNotes, flushPending])

  const pickFile = useCallback((event: React.ChangeEvent<HTMLInputElement>) => {
    const next = event.target.files?.[0] ?? null
    setFile(next)
    setState('pick')
    setErrors([])
    setScanNotes([])
    setUploadScanResult(null)
    setServerResponse(null)
  }, [])

  const runUpload = useCallback(async () => {
    if (!file) {
      setState('validation_failed')
      setErrors(['请先选择一个技能文件'])
      return
    }

    if (!isSupportedSkillPackage(file.name)) {
      setState('validation_failed')
      setErrors(['仅支持 .zip / .tar.gz / .tgz 技能包'])
      return
    }

    if (!selectedModelConfigId) {
      setState('validation_failed')
      setErrors(['请先选择一个启用中的模型配置'])
      return
    }

    setLoading(true)
    setErrors([])

    const selectedModel = modelOptions.find((item) => item.id === selectedModelConfigId)
    const scanModelLabel = selectedModel
      ? `${selectedModel.displayName} (${selectedModel.provider})`
      : '未选择'

    setState('scanning')
    setScanNotes([
      `读取上传包内容（${file.name}）`,
      '解析 SKILL.md frontmatter（name/description）',
      '扫描 SKILL.md 正文与脚本片段（注入/危险模式）',
      `启用 LLM 深度检测（模型=${scanModelLabel}）`,
      '解析 manifest.json 元数据（version/author）',
      '汇总风险等级并生成扫描报告',
    ])

    const formData = new FormData()
    formData.append('package', file)
    formData.append('enable_llm_scan', 'true')
    if (selectedModelConfigId) {
      formData.append('llm_model_config_id', selectedModelConfigId)
    }

    try {
      const res = await api.post('/skills/upload-package', formData, {
        headers: {
          'Content-Type': 'multipart/form-data',
        },
        timeout: 150_000,
      })
      const result = res.data as SkillUploadResponse
      const scanResult = mapScanResultPayload(result.scan_result)

      if (scanDone.current) {
        setServerResponse(result)
        setUploadScanResult(scanResult)
        setState('completed')
        setLoading(false)
        onSuccess()
      } else {
        pendingResult.current = { response: result, scanResult }
      }
    } catch (error: unknown) {
      const message = extractApiMessage(error, '上传失败')
      const parsedErrors = parseValidationErrors(message)
      const errList = parsedErrors.length > 0 ? parsedErrors : [message]
      if (scanDone.current) {
        setErrors(errList)
        setState('validation_failed')
        setLoading(false)
      } else {
        pendingResult.current = { errors: errList }
      }
    }
  }, [file, modelOptions, onSuccess, selectedModelConfigId])

  return {
    file,
    modelOptions,
    selectedModelConfigId,
    modelLoading,
    modelError,
    state,
    errors,
    scanNotes,
    scanProgress,
    loading,
    serverResponse,
    uploadScanResult,
    fileRef,
    pickFile,
    setSelectedModelConfigId,
    runUpload,
  }
}

function UploadSkillModal({ open, onOpenChange, onSuccess, onReviewRequest }: UploadSkillModalProps) {
  const {
    file,
    modelOptions,
    selectedModelConfigId,
    modelLoading,
    modelError,
    state,
    errors,
    scanNotes,
    scanProgress,
    loading,
    serverResponse,
    uploadScanResult,
    fileRef,
    pickFile,
    setSelectedModelConfigId,
    runUpload,
  } = useUploadSkillModalState(open, onSuccess)

  const closeModal = useCallback(() => {
    onOpenChange(false)
  }, [onOpenChange])

  const hasSelectedModel = selectedModelConfigId.length > 0
  const hasSelectedFile = file !== null
  const canSubmitUpload = hasSelectedFile && hasSelectedModel && !modelLoading
  const canReviewNow = state === 'completed' && serverResponse?.review_status === 'pending'

  let disabledTooltip = ''
  if (modelLoading) {
    disabledTooltip = '模型配置加载中，请稍候'
  } else if (!hasSelectedModel) {
    disabledTooltip = modelError || '请先选择启用中的 LLM 模型配置'
  } else if (!hasSelectedFile) {
    disabledTooltip = '请先选择技能包文件'
  }

  const handleReviewNow = useCallback(() => {
    if (!canReviewNow || !serverResponse) {
      return
    }

    onOpenChange(false)
    onReviewRequest({
      id: serverResponse.id,
      name: serverResponse.name,
      type: 'skill',
    })
  }, [canReviewNow, onOpenChange, onReviewRequest, serverResponse])

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[720px] rounded-none p-0 gap-0 ring-0" style={{ border: '1px solid #E8E8E8' }} showCloseButton={false}>
        <DialogHeader className="border-b border-[#E8E8E8] px-6 py-5">
          <div className="flex items-center justify-between">
            <DialogTitle className="flex items-center gap-2.5 text-base font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>
              <Upload className="h-4 w-4 text-[#0A6B3A]" />
              上传技能/插件扩展
            </DialogTitle>
            <button type="button" onClick={closeModal} className="font-mono text-sm font-semibold text-[#999999] hover:text-[#1A1A1A]">✕</button>
          </div>
          <DialogDescription className="sr-only">上传技能文件并查看校验与扫描状态</DialogDescription>
        </DialogHeader>

        <div className="space-y-4 px-6 py-6" style={{ maxHeight: '55vh', overflowY: 'auto' }}>
          {state === 'pick' && (
            <UploadPickPanel
              file={file}
              fileRef={fileRef}
              modelOptions={modelOptions}
              selectedModelConfigId={selectedModelConfigId}
              modelLoading={modelLoading}
              modelError={modelError}
              onPickFile={pickFile}
              onModelConfigChange={setSelectedModelConfigId}
            />
          )}
          {state !== 'pick' && (
            <UploadStatusPanel
              state={state}
              file={file}
              errors={errors}
              scanNotes={scanNotes}
              scanProgress={scanProgress}
              serverResponse={serverResponse}
              uploadScanResult={uploadScanResult}
            />
          )}
        </div>

        <UploadModalFooter
          state={state}
          loading={loading}
          canSubmit={canSubmitUpload}
          disabledTooltip={disabledTooltip}
          canReviewNow={canReviewNow}
          onCancel={closeModal}
          onSubmit={runUpload}
          onReviewNow={handleReviewNow}
        />
      </DialogContent>
    </Dialog>
  )
}

interface ReviewModalProps {
  target: ReviewTarget | null
  onClose: () => void
  onSuccess: () => void
}

interface ReviewScanSectionProps {
  scanLoading: boolean
  scanError: string
  scanResult: ScanResultPayload | null
  findings: ScanFinding[]
  canTriggerRescan: boolean
  rescanLoading: boolean
  rescanError: string
  onRescan: () => void
}

interface ReviewFooterProps {
  loading: boolean
  canSubmit: boolean
  onClose: () => void
  onApprove: () => void
  onReject: () => void
}

function mapScanResultPayload(row: unknown): ScanResultPayload | null {
  if (!row || typeof row !== 'object') {
    return null
  }

  const payload = row as Record<string, unknown>
  return {
    scanner_type: typeof payload.scanner_type === 'string' ? payload.scanner_type : 'unknown',
    verdict: typeof payload.verdict === 'string' ? payload.verdict : 'UNKNOWN',
    is_safe: payload.is_safe === true,
    max_severity: typeof payload.max_severity === 'string' ? payload.max_severity : undefined,
    findings_count: typeof payload.findings_count === 'number' ? payload.findings_count : 0,
    findings: normalizeFindings(payload.findings),
    scan_duration_ms: typeof payload.scan_duration_ms === 'number' ? payload.scan_duration_ms : undefined,
    scanned_at: typeof payload.scanned_at === 'string' ? payload.scanned_at : undefined,
    created_at: typeof payload.created_at === 'string' ? payload.created_at : new Date().toISOString(),
  }
}

function sortFindingsBySeverity(findings: ScanFinding[]): ScanFinding[] {
  const order: Record<string, number> = { CRITICAL: 1, HIGH: 2, MEDIUM: 3, LOW: 4, UNKNOWN: 5 }
  return [...findings].sort((left, right) => {
    const leftOrder = order[(left.severity ?? 'UNKNOWN').toUpperCase()] ?? 5
    const rightOrder = order[(right.severity ?? 'UNKNOWN').toUpperCase()] ?? 5
    return leftOrder - rightOrder
  })
}

function useReviewScanResult(target: ReviewTarget | null) {
  const [scanLoading, setScanLoading] = useState(false)
  const [scanError, setScanError] = useState('')
  const [scanResult, setScanResult] = useState<ScanResultPayload | null>(null)
  const requestSeqRef = useRef(0)

  const reload = useCallback(async () => {
    setScanError('')
    setScanResult(null)

    if (!target) {
      setScanLoading(false)
      return
    }

    requestSeqRef.current += 1
    const requestSeq = requestSeqRef.current
    setScanLoading(true)

    try {
      const res = await api.get(scanResultsEndpoint(target))
      if (requestSeq === requestSeqRef.current) {
        setScanResult(mapScanResultPayload(res.data?.scan_result))
      }
    } catch (error: unknown) {
      if (requestSeq === requestSeqRef.current) {
        setScanError(extractApiMessage(error, '扫描结果加载失败'))
      }
    } finally {
      if (requestSeq === requestSeqRef.current) {
        setScanLoading(false)
      }
    }
  }, [target])

  useEffect(() => {
    void reload()
  }, [reload])

  const findings = useMemo<ScanFinding[]>(() => {
    if (!scanResult) {
      return []
    }
    return sortFindingsBySeverity(scanResult.findings)
  }, [scanResult])

  return { scanLoading, scanError, scanResult, findings, reload }
}

function ReviewScanSection({
  scanLoading,
  scanError,
  scanResult,
  findings,
  canTriggerRescan,
  rescanLoading,
  rescanError,
  onRescan,
}: ReviewScanSectionProps) {
  return (
    <section className="space-y-3 border border-[#E8E8E8] bg-[#FAFAFA] p-4">
      <div className="flex items-center justify-between">
        <h3 className="font-mono text-[10px] font-semibold text-[#1A1A1A]">安全扫描结果</h3>
        {scanResult && <VerdictBadge verdict={scanResult.verdict} />}
      </div>

      {scanLoading && (
        <div className="flex items-center gap-2 font-mono text-[10px] text-[#999999]">
          <Loader2 className="h-3.5 w-3.5 animate-spin" />
          加载扫描结果...
        </div>
      )}

      {!scanLoading && scanError && <p className="font-mono text-[10px] text-[#CF1322]">{scanError}</p>}
      {!scanLoading && !scanError && !scanResult && (
        <div className="space-y-2">
          <p className="font-mono text-[10px] text-[#999999]">暂无扫描结果（可能是历史数据未执行扫描）</p>
          {canTriggerRescan && (
            <button
              type="button"
              disabled={rescanLoading}
              onClick={onRescan}
              className="inline-flex items-center gap-2 border border-[#E8E8E8] bg-white px-3 py-1.5 font-mono text-[9px] font-semibold text-[#1A1A1A] disabled:opacity-50"
            >
              {rescanLoading && <Loader2 className="h-3 w-3 animate-spin" />}
              立即重扫并刷新结果
            </button>
          )}
          {rescanError && <p className="font-mono text-[10px] text-[#CF1322]">{rescanError}</p>}
        </div>
      )}

      {!scanLoading && !scanError && scanResult && (
        <div className="space-y-2">
          {canTriggerRescan && (
            <button
              type="button"
              disabled={rescanLoading}
              onClick={onRescan}
              className="inline-flex items-center gap-2 border border-[#E8E8E8] bg-white px-3 py-1.5 font-mono text-[9px] font-semibold text-[#1A1A1A] disabled:opacity-50"
            >
              {rescanLoading && <Loader2 className="h-3 w-3 animate-spin" />}
              重新扫描
            </button>
          )}
          {rescanError && <p className="font-mono text-[10px] text-[#CF1322]">{rescanError}</p>}

          <div className="flex flex-wrap items-center gap-2 font-mono text-[10px] text-[#666666]">
            <span>扫描器：{scanResult.scanner_type}</span>
            <span>问题数：{scanResult.findings_count}</span>
            {scanResult.max_severity && <SeverityBadge severity={scanResult.max_severity} />}
          </div>

          {findings.length > 0 && (
            <div className="max-h-64 space-y-2 overflow-auto pr-1">
              {findings.map((finding, index) => (
                <details key={`${finding.rule_id ?? 'finding'}-${index}`} className="border border-[#E8E8E8] bg-white p-3">
                  <summary className="flex cursor-pointer items-center gap-2">
                    <SeverityBadge severity={finding.severity ?? 'UNKNOWN'} />
                    <span className="font-mono text-[10px] text-[#1A1A1A]">
                      {finding.title ?? finding.rule_id ?? `发现问题 #${index + 1}`}
                    </span>
                  </summary>
                  <div className="mt-2 space-y-1 font-mono text-[10px]">
                    {finding.rule_id && <p className="text-[#666666]">规则：{finding.rule_id}</p>}
                    {finding.location && <p className="text-[#666666]">位置：{finding.location}</p>}
                    {finding.snippet && (
                      <pre className="bg-[#F5F5F5] p-2 text-[9px] text-[#1A1A1A] whitespace-pre-wrap">
                        {finding.snippet}
                      </pre>
                    )}
                    {finding.recommendation && <p className="text-[#0A6B3A]">建议：{finding.recommendation}</p>}
                  </div>
                </details>
              ))}
            </div>
          )}
        </div>
      )}
    </section>
  )
}

function ReviewFooter({ loading, canSubmit, onClose, onApprove, onReject }: ReviewFooterProps) {
  return (
    <DialogFooter inset={false} className="flex-wrap items-center justify-end gap-3 border-[#E8E8E8] bg-white px-6 py-4" showCloseButton={false}>
      <button type="button" onClick={onClose} className="border border-[#E8E8E8] bg-white px-5 py-2.5 font-mono text-[10px] font-semibold text-[#1A1A1A]">
        取消
      </button>
      <button
        type="button"
        disabled={loading || !canSubmit}
        onClick={onReject}
        className="border border-[#CF132240] bg-[#CF132208] px-5 py-2.5 font-mono text-[10px] font-semibold text-[#CF1322] disabled:opacity-50"
      >
        拒绝
      </button>
      <button
        type="button"
        disabled={loading || !canSubmit}
        onClick={onApprove}
        className="flex items-center gap-2 bg-[#0A6B3A] px-5 py-2.5 font-mono text-[10px] font-semibold text-white disabled:opacity-50"
      >
        {loading && <Loader2 className="h-3 w-3 animate-spin" />}
        通过
      </button>
    </DialogFooter>
  )
}

function ReviewModal({ target, onClose, onSuccess }: ReviewModalProps) {
  const [note, setNote] = useState('')
  const [loading, setLoading] = useState(false)
  const [rescanLoading, setRescanLoading] = useState(false)
  const [rescanError, setRescanError] = useState('')
  const { scanLoading, scanError, scanResult, findings, reload } = useReviewScanResult(target)

  const open = target !== null

  useEffect(() => {
    setNote('')
    setRescanError('')
  }, [target])

  const triggerRescan = useCallback(async () => {
    if (!target || target.type !== 'skill') {
      return
    }

    setRescanLoading(true)
    setRescanError('')
    try {
      await api.post(`/skills/${target.id}/rescan`, {}, { timeout: 150_000 })
      await reload()
      toast.success('重扫完成并已刷新结果')
      onSuccess()
    } catch (error: unknown) {
      toast.error(extractApiMessage(error, '触发重扫失败'))
      setRescanError(extractApiMessage(error, '触发重扫失败'))
    } finally {
      setRescanLoading(false)
    }
  }, [onSuccess, reload, target])

  const canSubmitReview = !scanLoading && !!scanResult

  const submit = useCallback(async (approved: boolean) => {
    if (!target) {
      return
    }

    setLoading(true)
    try {
      await api.post(`/${target.type === 'skill' ? 'skills' : 'plugins'}/${target.id}/review`, { approved, note })
      toast.success(approved ? '审核已通过' : '审核已拒绝')
      onSuccess()
      onClose()
    } finally {
      setLoading(false)
    }
  }, [note, onClose, onSuccess, target])

  return (
    <Dialog open={open} onOpenChange={(nextOpen) => { if (!nextOpen) onClose() }}>
      <DialogContent className="sm:max-w-[760px] rounded-none p-0 gap-0 ring-0" style={{ border: '1px solid #E8E8E8' }} showCloseButton={false}>
        <DialogHeader className="border-b border-[#E8E8E8] px-6 py-5">
          <div className="flex items-center justify-between">
            <DialogTitle className="text-base font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>
              审核：{target?.name}
            </DialogTitle>
            <button type="button" onClick={onClose} className="font-mono text-sm font-semibold text-[#999999] hover:text-[#1A1A1A]">✕</button>
          </div>
          <DialogDescription className="sr-only">审核上传扩展并记录备注</DialogDescription>
        </DialogHeader>

        <div className="space-y-4 px-6 py-6" style={{ maxHeight: '55vh', overflowY: 'auto' }}>
          <ReviewScanSection
            scanLoading={scanLoading}
            scanError={scanError}
            scanResult={scanResult}
            findings={findings}
            canTriggerRescan={target?.type === 'skill' && !scanLoading}
            rescanLoading={rescanLoading}
            rescanError={rescanError}
            onRescan={triggerRescan}
          />

          <section className="space-y-2">
            <label className="font-mono text-[10px] font-semibold text-[#1A1A1A]" htmlFor="review-note">审核备注（可选）</label>
            <Textarea
              id="review-note"
              value={note}
              onChange={(event) => setNote(event.target.value)}
              rows={4}
              className="resize-none rounded-none border-[#E8E8E8] bg-[#F5F5F5] px-3.5 py-2.5 font-mono text-[10px] font-medium text-[#1A1A1A] placeholder:text-[#CCCCCC]"
            />
          </section>
        </div>

        <ReviewFooter
          loading={loading}
          canSubmit={canSubmitReview}
          onClose={onClose}
          onApprove={() => submit(true)}
          onReject={() => submit(false)}
        />
      </DialogContent>
    </Dialog>
  )
}

function SkillTable({
  items,
  loading,
  currentPage,
  onPageChange,
  onToggle,
  onReview,
  onRescan,
  onRelist,
  onYank,
}: {
  items: SkillItem[]
  loading: boolean
  currentPage: number
  onPageChange: (page: number) => void
  onToggle: (item: SkillItem) => void
  onReview: (item: SkillItem) => void
  onRescan: (item: SkillItem) => void
  onRelist: (item: SkillItem) => void
  onYank: (item: SkillItem) => void
}) {
  const pageSize = 10
  const pagedItems = useMemo(() => {
    const start = (currentPage - 1) * pageSize
    return items.slice(start, start + pageSize)
  }, [currentPage, items])

  return (
    <div className="overflow-hidden border border-[#E8E8E8] bg-white">
      <Table>
        <TableHeader>
          <TableRow className="h-10 bg-[#FAFAFA] hover:bg-[#FAFAFA]">
            <TableHead className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">扩展名</TableHead>
            <TableHead className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">描述</TableHead>
            <TableHead className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">来源</TableHead>
            <TableHead className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">审核状态</TableHead>
            <TableHead className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">版本</TableHead>
            <TableHead className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">调用次数</TableHead>
            <TableHead className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">审核信息</TableHead>
            <TableHead className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">启用</TableHead>
            <TableHead className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">操作</TableHead>
          </TableRow>
        </TableHeader>

        <TableBody>
          {loading && (
            <TableRow>
                <TableCell colSpan={9} className="py-10 text-center">
                <div className="inline-flex items-center gap-2 font-mono text-[10px] text-[#999999]">
                  <Loader2 className="h-4 w-4 animate-spin" />
                  加载中...
                </div>
              </TableCell>
            </TableRow>
          )}

          {!loading && pagedItems.length === 0 && (
            <TableRow>
              <TableCell colSpan={9} className="py-10 text-center font-mono text-[10px] text-[#999999]">暂无数据</TableCell>
            </TableRow>
          )}

          {!loading && pagedItems.map((item) => (
            <TableRow key={item.id} className="h-14">
              <TableCell className="max-w-56">
                <div className="flex items-center gap-2">
                  <span className="truncate font-mono text-[10px] font-semibold text-[#1A1A1A]">{item.name}</span>
                  {item.is_builtin && (
                    <span
                      className="inline-flex items-center px-1.5 py-0.5 font-mono text-[9px] font-semibold text-[#666666]"
                      style={{ backgroundColor: '#F0F0F0', border: '1px solid #E8E8E8' }}
                    >
                      内置
                    </span>
                  )}
                </div>
              </TableCell>
              <TableCell className="max-w-64 truncate font-mono text-[10px] text-[#999999]">{item.description || '—'}</TableCell>
              <TableCell><SourceBadge source={item.source} /></TableCell>
              <TableCell><ReviewBadge status={item.review_status} /></TableCell>
              <TableCell className="font-mono text-[10px] text-[#999999]">{item.version}</TableCell>
              <TableCell className="font-mono text-[10px] text-[#999999]">{item.invoke_count.toLocaleString()}</TableCell>
              <TableCell className="max-w-48 align-top">
                <ReviewHistoryCell item={item} />
              </TableCell>
              <TableCell><ToggleSwitch on={item.enabled} onChange={() => onToggle(item)} /></TableCell>
              <TableCell>
                <div className="flex items-center gap-2">
                  {item.review_status === 'pending' && (
                    <button type="button" onClick={() => onReview(item)} className={tableActionButtonClass()}>
                      审核
                    </button>
                  )}
                  {item.review_status === 'scan_failed' && (
                    <button type="button" onClick={() => onRescan(item)} className={tableActionButtonClass()}>
                      <RefreshCw className="h-3.5 w-3.5" />
                      重扫
                    </button>
                  )}
                  {item.review_status === 'approved' && (
                    <button type="button" onClick={() => onYank(item)} className={tableActionButtonClass('danger')}>
                      下架
                    </button>
                  )}
                  {item.review_status === 'yanked' && (
                    <button type="button" onClick={() => onRelist(item)} className={tableActionButtonClass()}>
                      上架
                    </button>
                  )}
                </div>
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>

      <TablePagination current={currentPage} total={items.length} pageSize={pageSize} onChange={onPageChange} />
    </div>
  )
}

function PluginTable({
  items,
  loading,
  currentPage,
  onPageChange,
  onToggle,
  onReview,
  onYank,
}: {
  items: PluginItem[]
  loading: boolean
  currentPage: number
  onPageChange: (page: number) => void
  onToggle: (item: PluginItem) => void
  onReview: (item: PluginItem) => void
  onYank: (item: PluginItem) => void
}) {
  const pageSize = 10
  const pagedItems = useMemo(() => {
    const start = (currentPage - 1) * pageSize
    return items.slice(start, start + pageSize)
  }, [currentPage, items])

  return (
    <div className="overflow-hidden border border-[#E8E8E8] bg-white">
      <Table>
        <TableHeader>
          <TableRow className="h-10 bg-[#FAFAFA] hover:bg-[#FAFAFA]">
            <TableHead className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">扩展名</TableHead>
            <TableHead className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">描述</TableHead>
            <TableHead className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">类型</TableHead>
            <TableHead className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">审核状态</TableHead>
            <TableHead className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">版本</TableHead>
            <TableHead className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">沙箱</TableHead>
            <TableHead className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">审核信息</TableHead>
            <TableHead className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">启用</TableHead>
            <TableHead className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">操作</TableHead>
          </TableRow>
        </TableHeader>

        <TableBody>
          {loading && (
            <TableRow>
                <TableCell colSpan={9} className="py-10 text-center">
                <div className="inline-flex items-center gap-2 font-mono text-[10px] text-[#999999]">
                  <Loader2 className="h-4 w-4 animate-spin" />
                  加载中...
                </div>
              </TableCell>
            </TableRow>
          )}

          {!loading && pagedItems.length === 0 && (
            <TableRow>
              <TableCell colSpan={9} className="py-10 text-center font-mono text-[10px] text-[#999999]">暂无数据</TableCell>
            </TableRow>
          )}

          {!loading && pagedItems.map((item) => (
            <TableRow key={item.id} className="h-14">
              <TableCell className="max-w-56 truncate font-mono text-[10px] font-semibold text-[#1A1A1A]">{item.name}</TableCell>
              <TableCell className="max-w-64 truncate font-mono text-[10px] text-[#999999]">{item.description || '—'}</TableCell>
              <TableCell>
                <span
                  className="inline-flex items-center px-1.5 py-0.5 font-mono text-[9px] font-semibold uppercase text-[#666666]"
                  style={{ backgroundColor: '#F0F0F0', border: '1px solid #E8E8E8' }}
                >
                  {item.plugin_type}
                </span>
              </TableCell>
              <TableCell><ReviewBadge status={item.review_status} /></TableCell>
              <TableCell className="font-mono text-[10px] text-[#999999]">{item.version}</TableCell>
              <TableCell className="font-mono text-[10px] text-[#999999]">{item.requires_sandbox ? '需要' : '—'}</TableCell>
              <TableCell className="max-w-48 align-top">
                <ReviewHistoryCell item={item} />
              </TableCell>
              <TableCell><ToggleSwitch on={item.enabled} onChange={() => onToggle(item)} /></TableCell>
              <TableCell>
                <div className="flex items-center gap-2">
                  {item.review_status === 'pending' && (
                    <button type="button" onClick={() => onReview(item)} className={tableActionButtonClass()}>
                      审核
                    </button>
                  )}
                  {item.review_status === 'approved' && (
                    <button type="button" onClick={() => onYank(item)} className={tableActionButtonClass('danger')}>
                      下架
                    </button>
                  )}
                </div>
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>

      <TablePagination current={currentPage} total={items.length} pageSize={pageSize} onChange={onPageChange} />
    </div>
  )
}

export default function ExtensionsPage() {
  const [activeTab, setActiveTab] = useState<TabKey>('skills')
  const [currentPage, setCurrentPage] = useState(1)
  const [skills, setSkills] = useState<SkillItem[]>([])
  const [plugins, setPlugins] = useState<PluginItem[]>([])
  const [loadingSkills, setLoadingSkills] = useState(false)
  const [loadingPlugins, setLoadingPlugins] = useState(false)
  const [uploadOpen, setUploadOpen] = useState(false)
  const [reviewFilter, setReviewFilter] = useState<ReviewFilter>('all')
  const [searchText, setSearchText] = useState('')
  const [reviewTarget, setReviewTarget] = useState<ReviewTarget | null>(null)

  const loadSkills = useCallback(async () => {
    setLoadingSkills(true)
    try {
      const res = await api.get('/skills')
      setSkills(Array.isArray(res.data.skills) ? res.data.skills : [])
    } catch (error) {
      console.error('加载技能列表失败', error)
    } finally {
      setLoadingSkills(false)
    }
  }, [])

  const loadPlugins = useCallback(async () => {
    setLoadingPlugins(true)
    try {
      const res = await api.get('/plugins')
      setPlugins(Array.isArray(res.data.plugins) ? res.data.plugins : [])
    } catch (error) {
      console.error('加载插件列表失败', error)
    } finally {
      setLoadingPlugins(false)
    }
  }, [])

  useEffect(() => {
    if (activeTab === 'skills') {
      loadSkills()
    } else {
      loadPlugins()
    }
  }, [activeTab, loadPlugins, loadSkills])

  useEffect(() => {
    setCurrentPage(1)
  }, [activeTab, reviewFilter, searchText])

  const handleToggleSkill = useCallback(async (item: SkillItem) => {
    const action = item.enabled ? 'disable' : 'enable'
    try {
      await api.post(`/skills/${item.id}/${action}`)
      toast.success(`技能已${action === 'enable' ? '启用' : '禁用'}`)
      await loadSkills()
    } catch (error) {
      toast.error(extractApiMessage(error, '切换技能状态失败'))
      console.error('切换技能状态失败', error)
    }
  }, [loadSkills])

  const handleTogglePlugin = useCallback(async (item: PluginItem) => {
    const action = item.enabled ? 'disable' : 'enable'
    try {
      await api.post(`/plugins/${item.id}/${action}`)
      toast.success(`插件已${action === 'enable' ? '启用' : '禁用'}`)
      await loadPlugins()
    } catch (error) {
      toast.error(extractApiMessage(error, '切换插件状态失败'))
      console.error('切换插件状态失败', error)
    }
  }, [loadPlugins])

  const handleRescanSkill = useCallback(async (item: SkillItem) => {
    try {
      await api.post(`/skills/${item.id}/rescan`, undefined, { timeout: 150_000 })
      toast.success('技能重扫完成')
      await loadSkills()
    } catch (error) {
      toast.error(extractApiMessage(error, '技能重扫失败'))
      console.error('技能重扫失败', error)
    }
  }, [loadSkills])

  const handleRelistSkill = useCallback(async (item: SkillItem) => {
    try {
      await api.post(`/skills/${item.id}/enable`)
      toast.success('技能已上架')
      await loadSkills()
    } catch (error) {
      toast.error(extractApiMessage(error, '技能上架失败'))
      console.error('技能上架失败', error)
    }
  }, [loadSkills])

  const handleYankSkill = useCallback(async (item: SkillItem) => {
    if (!window.confirm(`确认下架技能“${item.name}”吗？`)) {
      return
    }
    try {
      await api.post(`/skills/${item.id}/yank`, {})
      toast.success('技能已下架')
      await loadSkills()
    } catch (error) {
      toast.error(extractApiMessage(error, '技能下架失败'))
      console.error('技能下架失败', error)
    }
  }, [loadSkills])

  const handleYankPlugin = useCallback(async (item: PluginItem) => {
    if (!window.confirm(`确认下架插件“${item.name}”吗？`)) {
      return
    }
    try {
      await api.post(`/plugins/${item.id}/yank`, {})
      toast.success('插件已下架')
      await loadPlugins()
    } catch (error) {
      toast.error(extractApiMessage(error, '插件下架失败'))
      console.error('插件下架失败', error)
    }
  }, [loadPlugins])

  const activeList = activeTab === 'skills' ? skills : plugins
  const pendingSkills = useMemo(() => skills.filter((item) => item.review_status === 'pending').length, [skills])
  const pendingPlugins = useMemo(() => plugins.filter((item) => item.review_status === 'pending').length, [plugins])

  const filteredSkills = useMemo(() => {
    const keyword = searchText.trim().toLowerCase()
    return skills
      .filter((item) => matchesReviewFilter(item.review_status, reviewFilter))
      .filter((item) => {
        if (!keyword) {
          return true
        }
        return item.name.toLowerCase().includes(keyword) || (item.description ?? '').toLowerCase().includes(keyword)
      })
  }, [reviewFilter, searchText, skills])

  const filteredPlugins = useMemo(() => {
    const keyword = searchText.trim().toLowerCase()
    return plugins
      .filter((item) => matchesReviewFilter(item.review_status, reviewFilter))
      .filter((item) => {
        if (!keyword) {
          return true
        }
        return item.name.toLowerCase().includes(keyword) || (item.description ?? '').toLowerCase().includes(keyword)
      })
  }, [plugins, reviewFilter, searchText])

  return (
    <div className="mx-8 mt-7 max-w-[1136px] space-y-6">
      <header className="flex items-end justify-between">
        <div className="flex flex-col gap-2">
          <h1 className="text-[22px] font-bold tracking-tight text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px' }}>
            扩展管理
          </h1>
          <p className="font-mono text-xs text-[#999999]">管理可用技能与插件扩展，可审核并管理扩展包状态。</p>
        </div>
        {activeTab === 'skills' && (
          <button
            type="button"
            onClick={() => setUploadOpen(true)}
            className="flex items-center gap-2 bg-[#0A6B3A] px-4 py-2.5 font-mono text-[9px] font-semibold text-white"
          >
            <Upload className="h-3 w-3" />
            上传技能包
          </button>
        )}
      </header>

      <section className="flex gap-0" style={{ borderBottom: '1px solid #E8E8E8' }}>
        {([
          { key: 'skills', label: '技能管理', count: pendingSkills },
          { key: 'plugins', label: '插件管理', count: pendingPlugins },
        ] as const).map((tab) => (
          <button
            key={tab.key}
            type="button"
            onClick={() => setActiveTab(tab.key)}
            className="px-5 py-3 font-mono text-[10px] font-semibold"
            style={{
              color: activeTab === tab.key ? '#0A6B3A' : '#999999',
              borderBottom: activeTab === tab.key ? '2px solid #0A6B3A' : '2px solid transparent',
            }}
          >
            {tab.label}
            {tab.count > 0 && (
              <span className="ml-1 text-[9px] text-[#B45309]">({tab.count})</span>
            )}
          </button>
        ))}
      </section>

      <section style={{ borderBottom: '1px solid #E8E8E8' }}>
        <div className="flex h-10 items-center gap-0">
          {REVIEW_FILTER_OPTIONS.map((filter) => {
            const active = reviewFilter === filter
            const count = countByFilter(activeList, filter)
            return (
              <button
                key={filter}
                type="button"
                onClick={() => setReviewFilter(filter)}
                className="inline-flex h-10 min-w-[100px] items-center justify-center border-b-2 px-4 font-mono text-[10px] font-semibold"
                style={{
                  borderBottomColor: active ? '#0A6B3A' : 'transparent',
                  color: active ? '#0A6B3A' : '#999999',
                }}
              >
                {reviewFilterLabel(filter)} ({count})
              </button>
            )
          })}
        </div>
      </section>

      <section className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex flex-wrap items-center gap-2">
          <div className="flex w-[334px] items-center gap-2.5 border border-[#E8E8E8] bg-white px-3.5 py-2">
            <Search className="h-3 w-3 text-[#6a6a6a]" />
            <input
              value={searchText}
              onChange={(event) => setSearchText(event.target.value)}
              placeholder="搜索扩展名..."
              className="w-full bg-transparent font-mono text-[10px] font-medium text-[#1A1A1A] outline-none placeholder:text-[#CCCCCC]"
            />
          </div>
        </div>

        <p className="font-mono text-[10px] font-medium text-[#999999]">共 {countByFilter(activeList, reviewFilter)} 个扩展</p>
      </section>

      {activeTab === 'skills' && (
        <SkillTable
          items={filteredSkills}
          loading={loadingSkills}
          currentPage={currentPage}
          onPageChange={setCurrentPage}
          onToggle={handleToggleSkill}
          onReview={(item) => setReviewTarget({ id: item.id, name: item.name, type: 'skill' })}
          onRescan={handleRescanSkill}
          onRelist={handleRelistSkill}
          onYank={handleYankSkill}
        />
      )}

      {activeTab === 'plugins' && (
        <PluginTable
          items={filteredPlugins}
          loading={loadingPlugins}
          currentPage={currentPage}
          onPageChange={setCurrentPage}
          onToggle={handleTogglePlugin}
          onReview={(item) => setReviewTarget({ id: item.id, name: item.name, type: 'plugin' })}
          onYank={handleYankPlugin}
        />
      )}

      <UploadSkillModal
        open={uploadOpen}
        onOpenChange={setUploadOpen}
        onSuccess={loadSkills}
        onReviewRequest={(target) => {
          setUploadOpen(false)
          setReviewTarget(target)
        }}
      />
      <ReviewModal
        target={reviewTarget}
        onClose={() => setReviewTarget(null)}
        onSuccess={reviewTarget?.type === 'skill' ? loadSkills : loadPlugins}
      />
    </div>
  )
}