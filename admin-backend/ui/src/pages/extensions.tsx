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

type ReviewStatus = 'scanning' | 'pending' | 'approved' | 'rejected' | 'scan_failed' | 'yanked'
type ReviewFilter = 'all' | 'needs_scan' | 'pending' | 'approved' | 'rejected' | 'yanked'
type TabKey = 'skills' | 'plugins'
type UploadState = 'pick' | 'validation_failed' | 'scanning' | 'completed'

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
    const maybeResponse = (error as { response?: { data?: { message?: unknown } } }).response
    const maybeMessage = maybeResponse?.data?.message
    if (typeof maybeMessage === 'string' && maybeMessage.trim().length > 0) {
      return maybeMessage
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

function normalizeFindings(input: unknown): ScanFinding[] {
  if (!Array.isArray(input)) {
    return []
  }

  return input.reduce<ScanFinding[]>((acc, item) => {
    if (typeof item !== 'object' || item === null) {
      return acc
    }

    const row = item as Record<string, unknown>
    const locationPath = typeof row.location === 'string'
      ? row.location
      : typeof row.file === 'string'
        ? row.file
      : typeof row.file_path === 'string'
        ? row.file_path
        : undefined
    const line = typeof row.line === 'number' ? `:${row.line}` : ''

    acc.push({
      rule_id: typeof row.rule_id === 'string' ? row.rule_id : typeof row.id === 'string' ? row.id : undefined,
      severity: typeof row.severity === 'string'
        ? row.severity.toUpperCase()
        : typeof row.level === 'string'
          ? row.level.toUpperCase()
          : undefined,
      title: typeof row.title === 'string' ? row.title : typeof row.message === 'string' ? row.message : undefined,
      location: locationPath ? `${locationPath}${line}` : undefined,
      snippet: typeof row.code_snippet === 'string'
        ? row.code_snippet
        : typeof row.snippet === 'string'
          ? row.snippet
          : undefined,
      recommendation: typeof row.recommendation === 'string'
        ? row.recommendation
        : typeof row.suggestion === 'string'
          ? row.suggestion
          : undefined,
    })

    return acc
  }, [])
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
          <Loader2 className="h-3.5 w-3.5 animate-spin" />
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
}

interface UploadPickPanelProps {
  file: File | null
  fileRef: { current: HTMLInputElement | null }
  onPickFile: (event: React.ChangeEvent<HTMLInputElement>) => void
}

interface UploadStatusPanelProps {
  state: UploadState
  file: File | null
  errors: string[]
  scanNotes: string[]
  serverResponse: SkillUploadResponse | null
}

interface UploadModalFooterProps {
  state: UploadState
  loading: boolean
  onCancel: () => void
  onSubmit: () => void
}

function formatUploadFileLabel(file: File | null): string {
  if (!file) {
    return '尚未选择文件'
  }
  return `${file.name} (${(file.size / 1024 / 1024).toFixed(1)}MB)`
}

function UploadPickPanel({ file, fileRef, onPickFile }: UploadPickPanelProps) {
  return (
    <>
      <input
        ref={fileRef}
        type="file"
        accept=".zip,.tar.gz,.tgz"
        className="hidden"
        onChange={onPickFile}
      />
      <button
        type="button"
        onClick={() => fileRef.current?.click()}
        className="flex h-40 w-full items-center justify-center border border-dashed border-[#E8E8E8] bg-white px-4 font-mono text-[10px] font-medium text-[#999999] transition hover:border-[#0A6B3A] hover:text-[#0A6B3A]"
      >
        拖拽文件到此处，或点击选择 支持 .zip / .tar.gz 格式
      </button>
      <p className="font-mono text-[10px] font-medium text-[#1A1A1A]">待上传：{formatUploadFileLabel(file)}</p>
    </>
  )
}

function UploadStatusPanel({ state, file, errors, scanNotes, serverResponse }: UploadStatusPanelProps) {
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
        status={state === 'scanning' ? 'running' : state === 'completed' ? 'warn' : 'idle'}
        subtitle={state === 'scanning' ? '等待扫描步骤完成' : state === 'completed' ? '扫描已完成' : '等待前置步骤完成'}
      />

      {state === 'scanning' && (
        <div className="border border-[#1677FF40] bg-[#1677FF08] px-4 py-3">
          <ul className="space-y-1 font-mono text-[10px] text-[#1677FF]">
            {scanNotes.map((item, index) => (
              <li key={item} className="flex items-center gap-2">
                {index === 1 ? <Circle className="h-3.5 w-3.5 fill-[#1677FF] text-[#1677FF]" /> : <CheckCircle2 className="h-3.5 w-3.5 text-[#0A6B3A]" />}
                {item}
              </li>
            ))}
          </ul>
        </div>
      )}

      {state === 'completed' && serverResponse && (
        <div className="border border-[#D4870040] bg-[#D4870008] px-4 py-3 text-[#D48700]">
          <p className="font-mono text-[10px] font-semibold">
            结论：{serverResponse.review_status === 'scan_failed' ? 'DANGEROUS' : 'PENDING_REVIEW'}
          </p>
          <p className="font-mono text-[10px]">状态：{serverResponse.review_status}，等待管理员审核</p>
        </div>
      )}

      <StepRow
        index={4}
        title="提交审核队列"
        status={state === 'completed' ? 'done' : 'idle'}
        subtitle={state === 'completed' ? '状态：pending_review，等待管理员审核' : '等待前置步骤完成'}
      />
    </div>
  )
}

function UploadModalFooter({ state, loading, onCancel, onSubmit }: UploadModalFooterProps) {
  return (
    <DialogFooter inset={false} className="flex-wrap items-center justify-end gap-3 border-[#E8E8E8] bg-white px-6 py-4" showCloseButton={false}>
      <button type="button" onClick={onCancel} className="border border-[#E8E8E8] bg-white px-5 py-2.5 font-mono text-[10px] font-semibold text-[#1A1A1A]">
        取消
      </button>
      {state !== 'completed' ? (
        <button
          type="button"
          onClick={onSubmit}
          disabled={loading}
          className="flex items-center gap-2 bg-[#0A6B3A] px-5 py-2.5 font-mono text-[10px] font-semibold text-white disabled:opacity-50"
        >
          {loading ? <Loader2 className="h-3 w-3 animate-spin" /> : <CheckCircle2 className="h-3 w-3" />}
          开始上传
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

function useUploadSkillModalState(open: boolean, onSuccess: () => void) {
  const [file, setFile] = useState<File | null>(null)
  const [state, setState] = useState<UploadState>('pick')
  const [errors, setErrors] = useState<string[]>([])
  const [scanNotes, setScanNotes] = useState<string[]>([])
  const [loading, setLoading] = useState(false)
  const [serverResponse, setServerResponse] = useState<SkillUploadResponse | null>(null)
  const fileRef = useRef<HTMLInputElement | null>(null)

  useEffect(() => {
    if (open) {
      setFile(null)
      setState('pick')
      setErrors([])
      setScanNotes([])
      setLoading(false)
      setServerResponse(null)
    }
  }, [open])

  const pickFile = useCallback((event: React.ChangeEvent<HTMLInputElement>) => {
    const next = event.target.files?.[0] ?? null
    setFile(next)
    setState('pick')
    setErrors([])
    setScanNotes([])
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

    setLoading(true)
    setErrors([])

    setState('scanning')
    setScanNotes(['上传压缩包', '服务端提取 SKILL.md 并校验', '生成扫描报告'])

    const formData = new FormData()
    formData.append('package', file)

    try {
      const res = await api.post('/skills/upload-package', formData, {
        headers: {
          'Content-Type': 'multipart/form-data',
        },
      })
      const result = res.data as SkillUploadResponse
      setServerResponse(result)
      setState('completed')
      onSuccess()
    } catch (error: unknown) {
      const message = extractApiMessage(error, '上传失败')
      const parsedErrors = parseValidationErrors(message)
      setErrors(parsedErrors.length > 0 ? parsedErrors : [message])
      setState('validation_failed')
    } finally {
      setLoading(false)
    }
  }, [file, onSuccess])

  return {
    file,
    state,
    errors,
    scanNotes,
    loading,
    serverResponse,
    fileRef,
    pickFile,
    runUpload,
  }
}

function UploadSkillModal({ open, onOpenChange, onSuccess }: UploadSkillModalProps) {
  const {
    file,
    state,
    errors,
    scanNotes,
    loading,
    serverResponse,
    fileRef,
    pickFile,
    runUpload,
  } = useUploadSkillModalState(open, onSuccess)

  const closeModal = useCallback(() => {
    onOpenChange(false)
  }, [onOpenChange])

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
          {state === 'pick' && <UploadPickPanel file={file} fileRef={fileRef} onPickFile={pickFile} />}
          {state !== 'pick' && (
            <UploadStatusPanel
              state={state}
              file={file}
              errors={errors}
              scanNotes={scanNotes}
              serverResponse={serverResponse}
            />
          )}
        </div>

        <UploadModalFooter state={state} loading={loading} onCancel={closeModal} onSubmit={runUpload} />
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
}

interface ReviewFooterProps {
  loading: boolean
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

  useEffect(() => {
    setScanLoading(false)
    setScanError('')
    setScanResult(null)

    if (!target) {
      return
    }

    let disposed = false
    setScanLoading(true)

    api.get(scanResultsEndpoint(target))
      .then((res) => {
        if (disposed) {
          return
        }
        setScanResult(mapScanResultPayload(res.data?.scan_result))
      })
      .catch((error: unknown) => {
        if (disposed) {
          return
        }
        setScanError(extractApiMessage(error, '扫描结果加载失败'))
      })
      .finally(() => {
        if (!disposed) {
          setScanLoading(false)
        }
      })

    return () => {
      disposed = true
    }
  }, [target])

  const findings = useMemo<ScanFinding[]>(() => {
    if (!scanResult) {
      return []
    }
    return sortFindingsBySeverity(scanResult.findings)
  }, [scanResult])

  return { scanLoading, scanError, scanResult, findings }
}

function ReviewScanSection({ scanLoading, scanError, scanResult, findings }: ReviewScanSectionProps) {
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
      {!scanLoading && !scanError && !scanResult && <p className="font-mono text-[10px] text-[#999999]">暂无扫描结果</p>}

      {!scanLoading && !scanError && scanResult && (
        <div className="space-y-2">
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

function ReviewFooter({ loading, onClose, onApprove, onReject }: ReviewFooterProps) {
  return (
    <DialogFooter inset={false} className="flex-wrap items-center justify-end gap-3 border-[#E8E8E8] bg-white px-6 py-4" showCloseButton={false}>
      <button type="button" onClick={onClose} className="border border-[#E8E8E8] bg-white px-5 py-2.5 font-mono text-[10px] font-semibold text-[#1A1A1A]">
        取消
      </button>
      <button
        type="button"
        disabled={loading}
        onClick={onReject}
        className="border border-[#CF132240] bg-[#CF132208] px-5 py-2.5 font-mono text-[10px] font-semibold text-[#CF1322] disabled:opacity-50"
      >
        拒绝
      </button>
      <button
        type="button"
        disabled={loading}
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
  const { scanLoading, scanError, scanResult, findings } = useReviewScanResult(target)

  const open = target !== null

  useEffect(() => {
    setNote('')
  }, [target])

  const submit = useCallback(async (approved: boolean) => {
    if (!target) {
      return
    }

    setLoading(true)
    try {
      await api.post(`/${target.type === 'skill' ? 'skills' : 'plugins'}/${target.id}/review`, { approved, note })
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
          <ReviewScanSection scanLoading={scanLoading} scanError={scanError} scanResult={scanResult} findings={findings} />

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
  onYank,
}: {
  items: SkillItem[]
  loading: boolean
  currentPage: number
  onPageChange: (page: number) => void
  onToggle: (item: SkillItem) => void
  onReview: (item: SkillItem) => void
  onRescan: (item: SkillItem) => void
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
              <TableCell className="max-w-48 truncate font-mono text-[10px] text-[#999999]">{reviewHistoryLabel(item)}</TableCell>
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
              <TableCell className="max-w-48 truncate font-mono text-[10px] text-[#999999]">{reviewHistoryLabel(item)}</TableCell>
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
      await loadSkills()
    } catch (error) {
      console.error('切换技能状态失败', error)
    }
  }, [loadSkills])

  const handleTogglePlugin = useCallback(async (item: PluginItem) => {
    const action = item.enabled ? 'disable' : 'enable'
    try {
      await api.post(`/plugins/${item.id}/${action}`)
      await loadPlugins()
    } catch (error) {
      console.error('切换插件状态失败', error)
    }
  }, [loadPlugins])

  const handleRescanSkill = useCallback(async (item: SkillItem) => {
    try {
      await api.post(`/skills/${item.id}/rescan`)
      await loadSkills()
    } catch (error) {
      console.error('技能重扫失败', error)
    }
  }, [loadSkills])

  const handleYankSkill = useCallback(async (item: SkillItem) => {
    if (!window.confirm(`确认下架技能“${item.name}”吗？`)) {
      return
    }
    try {
      await api.post(`/skills/${item.id}/yank`, {})
      await loadSkills()
    } catch (error) {
      console.error('技能下架失败', error)
    }
  }, [loadSkills])

  const handleYankPlugin = useCallback(async (item: PluginItem) => {
    if (!window.confirm(`确认下架插件“${item.name}”吗？`)) {
      return
    }
    try {
      await api.post(`/plugins/${item.id}/yank`, {})
      await loadPlugins()
    } catch (error) {
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

      <UploadSkillModal open={uploadOpen} onOpenChange={setUploadOpen} onSuccess={loadSkills} />
      <ReviewModal
        target={reviewTarget}
        onClose={() => setReviewTarget(null)}
        onSuccess={reviewTarget?.type === 'skill' ? loadSkills : loadPlugins}
      />
    </div>
  )
}