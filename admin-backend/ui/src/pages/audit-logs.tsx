import { Search, Download, Loader2 } from 'lucide-react'
import { useState, useEffect, useCallback } from 'react'
import { api } from '@/lib/api'
import { TablePagination } from '@/components/ui/table-pagination'
import {
  Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription,
} from '@/components/ui/dialog'

/* ── 常量 ── */

const PAGE_SIZE_OPTIONS = [10, 20, 50, 100]

const ACTION_OPTIONS = [
  { value: '', label: '全部操作' },
  { value: 'login_success', label: '登录成功' },
  { value: 'login_failed', label: '登录失败' },
  { value: 'create_user', label: '创建用户' },
  { value: 'update_user', label: '更新用户' },
  { value: 'delete_user', label: '删除用户' },
  { value: 'create_role', label: '创建角色' },
  { value: 'update_role', label: '更新角色' },
  { value: 'delete_role', label: '删除角色' },
  { value: 'assign_permissions', label: '分配权限' },
]

const REPORT_TYPE_OPTIONS = [
  { value: '', label: '全部类型' },
  { value: 'audit_log', label: '审计日志' },
  { value: 'dlp_event', label: 'DLP 事件' },
  { value: 'usage_stats', label: '使用统计' },
]

type TabKey = '操作审计' | '客户端事件'
const TABS: TabKey[] = ['操作审计', '客户端事件']

/* ── 类型 ── */

interface AuditLog {
  id: string
  username: string | null
  user_id: string | null
  action: string
  details: string
  ip_address: string | null
  user_agent: string | null
  created_at: string
}

interface ClientEvent {
  id: string
  report_type: string
  payload: unknown
  received_at: string
  client_username: string | null
}

interface Filters {
  q: string
  action: string
  startDate: string
  endDate: string
}

interface EventFilters {
  report_type: string
  startDate: string
  endDate: string
}

/* ── 操作类型颜色 ── */

const actionColorMap: Record<string, string> = {
  create: '#0A6B3A',
  update: '#D48700',
  delete: '#CF1322',
  login: '#1677FF',
  export: '#D4B200',
  toggle: '#D48700',
  enable: '#0A6B3A',
  disable: '#D48700',
  block: '#CF1322',
  scan: '#1677FF',
}

function getActionColor(action: string): string {
  const lower = action.toLowerCase()
  for (const [key, color] of Object.entries(actionColorMap)) {
    if (lower.includes(key)) return color
  }
  return '#1A1A1A'
}

const actionLabelMap: Record<string, string> = Object.fromEntries(
  ACTION_OPTIONS.filter(o => o.value).map(o => [o.value, o.label])
)

function getActionLabel(action: string): string {
  return actionLabelMap[action] ?? action
}

const reportTypeColorMap: Record<string, string> = {
  audit_log: '#1677FF',
  dlp_event: '#CF1322',
  health_status: '#0A6B3A',
  usage_stats: '#D48700',
}

function getReportTypeColor(type: string): string {
  return reportTypeColorMap[type] ?? '#1A1A1A'
}

/* ── 子组件: 审计日志详情 Dialog ── */

const DETAIL_FIELDS: Array<[string, keyof AuditLog]> = [
  ['日志 ID', 'id'],
  ['操作人', 'username'],
  ['操作类型', 'action'],
  ['详情', 'details'],
  ['来源 IP', 'ip_address'],
  ['User-Agent', 'user_agent'],
  ['时间', 'created_at'],
]

function LogDetailDialog({ log, onClose }: { log: AuditLog | null; onClose: () => void }) {
  return (
    <Dialog open={!!log} onOpenChange={(open) => { if (!open) onClose() }}>
      <DialogContent className="sm:max-w-[560px] rounded-none p-0 gap-0 ring-0" style={{ border: '1px solid #E8E8E8' }} showCloseButton={false}>
        <DialogHeader className="border-b border-[#E8E8E8] px-6 py-5">
          <div className="flex items-center justify-between">
            <DialogTitle className="text-base font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>
              日志详情
            </DialogTitle>
            <button type="button" onClick={onClose} className="font-mono text-sm font-semibold text-[#999999] hover:text-[#1A1A1A]">✕</button>
          </div>
          <DialogDescription className="sr-only">审计日志详细信息</DialogDescription>
        </DialogHeader>
        {log && (
          <div className="flex flex-col gap-4 px-6 py-6">
            {DETAIL_FIELDS.map(([label, key]) => {
              const raw = log[key]
              const value = key === 'created_at'
                ? new Date(raw as string).toLocaleString('zh-CN')
                : key === 'action'
                  ? getActionLabel(raw as string)
                  : (raw ?? '—')
              return (
                <div key={label} className="flex flex-col gap-1">
                  <span className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{label}</span>
                  <span className="font-mono text-[10px] text-[#1A1A1A] break-all">{value}</span>
                </div>
              )
            })}
          </div>
        )}
      </DialogContent>
    </Dialog>
  )
}

/* ── 子组件: 客户端事件详情 Dialog ── */

function ClientEventDialog({ event, onClose }: { event: ClientEvent | null; onClose: () => void }) {
  return (
    <Dialog open={!!event} onOpenChange={(open) => { if (!open) onClose() }}>
      <DialogContent className="sm:max-w-[600px] rounded-none p-0 gap-0 ring-0" style={{ border: '1px solid #E8E8E8' }} showCloseButton={false}>
        <DialogHeader className="border-b border-[#E8E8E8] px-6 py-5">
          <div className="flex items-center justify-between">
            <DialogTitle className="text-base font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>
              事件详情
            </DialogTitle>
            <button type="button" onClick={onClose} className="font-mono text-sm font-semibold text-[#999999] hover:text-[#1A1A1A]">✕</button>
          </div>
          <DialogDescription className="sr-only">客户端事件详细信息</DialogDescription>
        </DialogHeader>
        {event && (
          <div className="flex flex-col gap-4 px-6 py-6">
            <div className="flex flex-col gap-1">
              <span className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">事件 ID</span>
              <span className="font-mono text-[10px] text-[#1A1A1A] break-all">{event.id}</span>
            </div>
            <div className="flex flex-col gap-1">
              <span className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">来源客户端</span>
              <span className="font-mono text-[10px] text-[#1A1A1A]">{event.client_username ?? '—'}</span>
            </div>
            <div className="flex flex-col gap-1">
              <span className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">事件类型</span>
              <span className="font-mono text-[10px] font-semibold" style={{ color: getReportTypeColor(event.report_type) }}>{event.report_type}</span>
            </div>
            <div className="flex flex-col gap-1">
              <span className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">时间</span>
              <span className="font-mono text-[10px] text-[#1A1A1A]">{new Date(event.received_at).toLocaleString('zh-CN')}</span>
            </div>
            <div className="flex flex-col gap-1">
              <span className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">事件载荷 (JSON)</span>
              <pre className="rounded bg-[#F5F5F5] p-3 font-mono text-[9px] text-[#1A1A1A] overflow-auto max-h-[280px] whitespace-pre-wrap break-all">
                {JSON.stringify(event.payload, null, 2)}
              </pre>
            </div>
          </div>
        )}
      </DialogContent>
    </Dialog>
  )
}

/* ── 子组件: 客户端事件 Tab ── */

function ClientEventsTab() {
  const [events, setEvents] = useState<ClientEvent[]>([])
  const [total, setTotal] = useState(0)
  const [page, setPage] = useState(1)
  const [pageSize, setPageSize] = useState(20)
  const [loading, setLoading] = useState(true)
  const [filters, setFilters] = useState<EventFilters>({ report_type: '', startDate: '', endDate: '' })
  const [selectedEvent, setSelectedEvent] = useState<ClientEvent | null>(null)

  const loadEvents = useCallback(async (currentPage: number, f: EventFilters, ps: number) => {
    setLoading(true)
    try {
      const params: Record<string, string | number | boolean> = {
        page: currentPage,
        page_size: ps,
        include_health_status: false,
      }
      if (f.report_type) params.report_type = f.report_type
      if (f.startDate) params.start_time = `${f.startDate}T00:00:00Z`
      if (f.endDate) params.end_time = `${f.endDate}T23:59:59Z`
      const res = await api.get('/client-events', { params })
      setEvents(res.data.events ?? [])
      setTotal(res.data.total ?? 0)
    } catch (err) {
      console.error('加载客户端事件失败', err)
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => { loadEvents(page, filters, pageSize) }, [page, filters, pageSize, loadEvents])

  const handleFilterChange = useCallback((key: keyof EventFilters, value: string) => {
    setFilters(prev => ({ ...prev, [key]: value }))
    setPage(1)
  }, [])

  return (
    <>
      <div className="flex flex-wrap items-center gap-2">
        <select
          value={filters.report_type}
          onChange={(e) => handleFilterChange('report_type', e.target.value)}
          className="border border-[#E8E8E8] bg-white px-3 py-2 font-mono text-[10px] font-medium text-[#1A1A1A] outline-none"
        >
          {REPORT_TYPE_OPTIONS.map(opt => (
            <option key={opt.value} value={opt.value}>{opt.label}</option>
          ))}
        </select>
        <input
          type="date"
          value={filters.startDate}
          onChange={(e) => handleFilterChange('startDate', e.target.value)}
          className="border border-[#E8E8E8] bg-white px-3 py-2 font-mono text-[10px] font-medium text-[#1A1A1A] outline-none"
          title="开始日期"
        />
        <span className="font-mono text-[10px] text-[#999999]">—</span>
        <input
          type="date"
          value={filters.endDate}
          onChange={(e) => handleFilterChange('endDate', e.target.value)}
          className="border border-[#E8E8E8] bg-white px-3 py-2 font-mono text-[10px] font-medium text-[#1A1A1A] outline-none"
          title="结束日期"
        />
      </div>

      <div className="bg-white" style={{ border: '1px solid #E8E8E8' }}>
        <div className="grid grid-cols-5 px-4 py-2.5" style={{ borderBottom: '1px solid #E8E8E8' }}>
          {['时间', '来源客户端', '事件类型', '载荷摘要', '操作'].map((h) => (
            <span key={h} className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{h}</span>
          ))}
        </div>
        {loading ? (
          <div className="flex items-center justify-center py-10">
            <Loader2 className="h-5 w-5 animate-spin text-[#999999]" />
          </div>
        ) : events.length === 0 ? (
          <div className="flex items-center justify-center py-10">
            <span className="font-mono text-[10px] text-[#CCCCCC]">暂无客户端事件</span>
          </div>
        ) : (
          events.map((ev, i) => (
            <div
              key={ev.id}
              className="grid grid-cols-5 items-center px-4 py-3"
              style={{ borderBottom: i < events.length - 1 ? '1px solid #E8E8E8' : 'none' }}
            >
              <span className="font-mono text-[10px] font-medium text-[#999999]">
                {new Date(ev.received_at).toLocaleString('zh-CN', { year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', second: '2-digit' })}
              </span>
              <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{ev.client_username ?? '—'}</span>
              <span className="font-mono text-[10px] font-semibold" style={{ color: getReportTypeColor(ev.report_type) }}>
                {ev.report_type}
              </span>
              <span className="font-mono text-[10px] font-medium text-[#999999] truncate" title={JSON.stringify(ev.payload)}>
                {typeof ev.payload === 'object' && ev.payload !== null
                  ? Object.keys(ev.payload as Record<string, unknown>).slice(0, 3).join(', ')
                  : String(ev.payload ?? '—')}
              </span>
              <button
                className="font-mono text-[10px] font-semibold text-[#0A6B3A] text-left"
                onClick={() => setSelectedEvent(ev)}
              >
                详情
              </button>
            </div>
          ))
        )}
        <TablePagination current={page} total={total} pageSize={pageSize} onChange={setPage} />
        <div className="flex items-center justify-end gap-2 px-4 py-2" style={{ borderTop: '1px solid #E8E8E8' }}>
          <span className="font-mono text-[10px] text-[#999999]">每页</span>
          <select
            value={pageSize}
            onChange={(e) => { setPageSize(Number(e.target.value)); setPage(1) }}
            className="border border-[#E8E8E8] bg-white px-2 py-1 font-mono text-[10px] font-medium text-[#1A1A1A] outline-none"
          >
            {PAGE_SIZE_OPTIONS.map(n => (
              <option key={n} value={n}>{n} 条</option>
            ))}
          </select>
        </div>
      </div>

      <ClientEventDialog event={selectedEvent} onClose={() => setSelectedEvent(null)} />
    </>
  )
}

/* ── 主组件 ── */

export default function AuditLogsPage() {
  const [activeTab, setActiveTab] = useState<TabKey>('操作审计')
  const [logs, setLogs] = useState<AuditLog[]>([])
  const [total, setTotal] = useState(0)
  const [page, setPage] = useState(1)
  const [pageSize, setPageSize] = useState(10)
  const [loading, setLoading] = useState(true)
  const [filters, setFilters] = useState<Filters>({ q: '', action: '', startDate: '', endDate: '' })
  const [pendingQ, setPendingQ] = useState('')
  const [selectedLog, setSelectedLog] = useState<AuditLog | null>(null)

  const loadLogs = useCallback(async (currentPage: number, f: Filters, ps: number) => {
    setLoading(true)
    try {
      const params: Record<string, string | number> = { page: currentPage, page_size: ps }
      if (f.q) params.q = f.q
      if (f.action) params.action = f.action
      if (f.startDate) params.start_time = `${f.startDate}T00:00:00Z`
      if (f.endDate) params.end_time = `${f.endDate}T23:59:59Z`
      const res = await api.get('/audit-logs', { params })
      setLogs(res.data.logs ?? [])
      setTotal(res.data.total ?? 0)
    } catch (err) {
      console.error('加载审计日志失败', err)
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    if (activeTab === '操作审计') loadLogs(page, filters, pageSize)
  }, [page, filters, pageSize, loadLogs, activeTab])

  /* 搜索文本 300ms 防抖后触发服务端查询 */
  useEffect(() => {
    const timer = setTimeout(() => {
      setFilters(prev => ({ ...prev, q: pendingQ }))
      setPage(1)
    }, 300)
    return () => clearTimeout(timer)
  }, [pendingQ])

  const handleFilterChange = useCallback((key: keyof Omit<Filters, 'q'>, value: string) => {
    setFilters(prev => ({ ...prev, [key]: value }))
    setPage(1)
  }, [])

  const handleExport = useCallback(async () => {
    try {
      const params: Record<string, string> = {}
      if (filters.q) params.q = filters.q
      if (filters.action) params.action = filters.action
      if (filters.startDate) params.start_time = `${filters.startDate}T00:00:00Z`
      if (filters.endDate) params.end_time = `${filters.endDate}T23:59:59Z`
      const res = await api.get('/audit-logs/export', { params, responseType: 'blob' })
      const blob = new Blob([res.data], { type: 'text/csv;charset=utf-8' })
      const url = URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = `audit-logs-${new Date().toISOString().slice(0, 10)}.csv`
      a.click()
      URL.revokeObjectURL(url)
    } catch (err) {
      console.error('导出审计日志失败', err)
    }
  }, [filters])

  return (
    <div className="flex flex-col gap-6">
      <div className="flex items-end justify-between">
        <div className="flex flex-col gap-2">
          <h1 className="text-[22px] font-bold tracking-tight text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px' }}>
            审计日志
          </h1>
          <p className="font-mono text-xs text-[#999999]">查看和导出所有管理操作的审计记录</p>
        </div>
      </div>

      {/* Tab 栏 */}
      <div className="flex gap-0" style={{ borderBottom: '1px solid #E8E8E8' }}>
        {TABS.map((tab) => (
          <button
            key={tab}
            onClick={() => setActiveTab(tab)}
            className="px-5 py-3 font-mono text-[10px] font-semibold"
            style={{
              color: activeTab === tab ? '#0A6B3A' : '#999999',
              borderBottom: activeTab === tab ? '2px solid #0A6B3A' : '2px solid transparent',
            }}
          >
            {tab}
          </button>
        ))}
      </div>

      {activeTab === '操作审计' && (
        <>
          {/* 筛选栏 */}
          <div className="flex flex-wrap items-center justify-between gap-2">
            <div className="flex flex-wrap items-center gap-2">
              <div className="flex w-[200px] items-center gap-2.5 border border-[#E8E8E8] bg-white px-3.5 py-2">
                <Search className="h-3 w-3 shrink-0 text-[#6a6a6a]" />
                <input
                  type="text"
                  value={pendingQ}
                  onChange={(e) => setPendingQ(e.target.value)}
                  placeholder="搜索操作或详情..."
                  className="w-full bg-transparent font-mono text-[10px] font-medium text-[#1A1A1A] placeholder:text-[#CCCCCC] outline-none"
                />
              </div>
              <select
                value={filters.action}
                onChange={(e) => handleFilterChange('action', e.target.value)}
                className="border border-[#E8E8E8] bg-white px-3 py-2 font-mono text-[10px] font-medium text-[#1A1A1A] outline-none"
              >
                {ACTION_OPTIONS.map(opt => (
                  <option key={opt.value} value={opt.value}>{opt.label}</option>
                ))}
              </select>
              <input
                type="date"
                value={filters.startDate}
                onChange={(e) => handleFilterChange('startDate', e.target.value)}
                className="border border-[#E8E8E8] bg-white px-3 py-2 font-mono text-[10px] font-medium text-[#1A1A1A] outline-none"
                title="开始日期"
              />
              <span className="font-mono text-[10px] text-[#999999]">—</span>
              <input
                type="date"
                value={filters.endDate}
                onChange={(e) => handleFilterChange('endDate', e.target.value)}
                className="border border-[#E8E8E8] bg-white px-3 py-2 font-mono text-[10px] font-medium text-[#1A1A1A] outline-none"
                title="结束日期"
              />
            </div>
            <button
              className="flex items-center gap-2 bg-[#0A6B3A] px-4 py-2.5 font-mono text-[9px] font-semibold text-white"
              onClick={handleExport}
            >
              <Download className="h-3 w-3" />
              导出 CSV
            </button>
          </div>

          {/* 数据表格 */}
          <div className="bg-white" style={{ border: '1px solid #E8E8E8' }}>
            <div className="grid grid-cols-6 px-4 py-2.5" style={{ borderBottom: '1px solid #E8E8E8' }}>
              {['时间', '操作人', '操作类型', '详情', 'IP 地址', '操作'].map((h) => (
                <span key={h} className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{h}</span>
              ))}
            </div>
            {loading ? (
              <div className="flex items-center justify-center py-10">
                <Loader2 className="h-5 w-5 animate-spin text-[#999999]" />
              </div>
            ) : logs.length === 0 ? (
              <div className="flex items-center justify-center py-10">
                <span className="font-mono text-[10px] text-[#CCCCCC]">暂无审计日志</span>
              </div>
            ) : (
              logs.map((log, i) => (
                <div
                  key={log.id}
                  className="grid grid-cols-6 items-center px-4 py-3"
                  style={{ borderBottom: i < logs.length - 1 ? '1px solid #E8E8E8' : 'none' }}
                >
                  <span className="font-mono text-[10px] font-medium text-[#999999]">
                    {new Date(log.created_at).toLocaleString('zh-CN', { year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', second: '2-digit' })}
                  </span>
                  <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{log.username ?? '系统'}</span>
                  <span className="font-mono text-[10px] font-semibold" style={{ color: getActionColor(log.action) }}>
                    {getActionLabel(log.action)}
                  </span>
                  <span className="font-mono text-[10px] font-medium text-[#999999] truncate" title={log.details}>{log.details}</span>
                  <span className="font-mono text-[10px] font-medium text-[#999999]">{log.ip_address ?? '—'}</span>
                  <button
                    className="font-mono text-[10px] font-semibold text-[#0A6B3A] text-left"
                    onClick={() => setSelectedLog(log)}
                  >
                    详情
                  </button>
                </div>
              ))
            )}
            <TablePagination current={page} total={total} pageSize={pageSize} onChange={setPage} />
            <div className="flex items-center justify-end gap-2 px-4 py-2" style={{ borderTop: '1px solid #E8E8E8' }}>
              <span className="font-mono text-[10px] text-[#999999]">每页</span>
              <select
                value={pageSize}
                onChange={(e) => { setPageSize(Number(e.target.value)); setPage(1) }}
                className="border border-[#E8E8E8] bg-white px-2 py-1 font-mono text-[10px] font-medium text-[#1A1A1A] outline-none"
              >
                {PAGE_SIZE_OPTIONS.map(n => (
                  <option key={n} value={n}>{n} 条</option>
                ))}
              </select>
            </div>
          </div>

          <LogDetailDialog log={selectedLog} onClose={() => setSelectedLog(null)} />
        </>
      )}

      {activeTab === '客户端事件' && <ClientEventsTab />}
    </div>
  )
}
