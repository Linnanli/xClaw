import { Search, Bell, Plus, Loader2, X } from 'lucide-react'
import { useState, useEffect, useCallback } from 'react'
import { api } from '@/lib/api'
import { TablePagination } from '@/components/ui/table-pagination'
import { StatusTag } from '@/components/ui/status-tag'
import { Dialog, DialogContent } from '@/components/ui/dialog'
import { FormLabel } from '@/components/ui/form-helpers'
import type { AlertEvent, AlertRule, AlertStats } from '@/types'

/* ── 常量映射 ── */

const severityMap: Record<string, { label: string; color: string }> = {
  critical: { label: '严重', color: '#CF1322' },
  high: { label: '高危', color: '#D48700' },
  medium: { label: '中危', color: '#D4B200' },
  low: { label: '低危', color: '#0A6B3A' },
}

const statusMap: Record<string, { label: string; color: string }> = {
  pending: { label: '未处理', color: '#CF1322' },
  acknowledged: { label: '已确认', color: '#D48700' },
  in_progress: { label: '处理中', color: '#D48700' },
  closed: { label: '已关闭', color: '#0A6B3A' },
}

const eventTypeLabels: Record<string, string> = {
  dlp_violation: 'DLP 拦截',
  quota_exceeded: '配额超限',
  model_error: '模型异常',
  abnormal_login: '异常登录',
  approval_timeout: '审批超时',
}

const EVENT_TYPES = Object.keys(eventTypeLabels)
const SEVERITIES = ['low', 'medium', 'high', 'critical']
const CHANNELS = ['email', 'wecom', 'dingtalk', 'feishu']

const inputClass = 'border border-[#E8E8E8] px-3 py-2 font-mono text-[10px] outline-none'

/* ── 主组件 ── */

export default function AlertsPage() {
  const [events, setEvents] = useState<AlertEvent[]>([])
  const [stats, setStats] = useState<AlertStats>({ pending: 0, in_progress: 0, today: 0, closed: 0 })
  const [total, setTotal] = useState(0)
  const [page, setPage] = useState(1)
  const [loading, setLoading] = useState(true)
  const [filterSeverity, setFilterSeverity] = useState('')
  const [filterStatus, setFilterStatus] = useState('')
  const [search, setSearch] = useState('')

  // 弹窗状态
  const [ruleDialogOpen, setRuleDialogOpen] = useState(false)
  const [rulesListOpen, setRulesListOpen] = useState(false)
  const [statusDialogOpen, setStatusDialogOpen] = useState(false)
  const [selectedEvent, setSelectedEvent] = useState<AlertEvent | null>(null)

  const pageSize = 20

  const fetchEvents = useCallback(async () => {
    setLoading(true)
    try {
      const params = new URLSearchParams({ page: String(page), page_size: String(pageSize) })
      if (filterSeverity) params.set('severity', filterSeverity)
      if (filterStatus) params.set('status', filterStatus)
      if (search.trim()) params.set('search', search.trim())
      const { data } = await api.get(`/alerts?${params}`)
      setEvents(data.data)
      setTotal(data.total)
    } catch { /* 静默 */ }
    setLoading(false)
  }, [page, filterSeverity, filterStatus, search])

  const fetchStats = useCallback(async () => {
    try {
      const { data } = await api.get('/alerts/stats')
      setStats(data)
    } catch { /* 静默 */ }
  }, [])

  useEffect(() => { fetchEvents() }, [fetchEvents])
  useEffect(() => { fetchStats() }, [fetchStats])

  const handleStatusUpdate = async (status: string, note: string) => {
    if (!selectedEvent) return
    try {
      await api.put(`/alerts/${selectedEvent.id}/status`, { status, note: note || undefined })
      setStatusDialogOpen(false)
      setSelectedEvent(null)
      fetchEvents()
      fetchStats()
    } catch { /* 静默 */ }
  }

  const statCards = [
    { label: '未处理', value: stats.pending, dotColor: '#CF1322', valueColor: '#CF1322', borderColor: 'rgba(207,19,34,0.25)' },
    { label: '处理中', value: stats.in_progress, dotColor: '', valueColor: '#D48700', borderColor: 'rgba(212,135,0,0.25)' },
    { label: '今日告警', value: stats.today, dotColor: '', valueColor: '#1A1A1A', borderColor: '#E8E8E8' },
    { label: '已关闭', value: stats.closed, dotColor: '', valueColor: '#1A1A1A', borderColor: '#E8E8E8' },
  ]

  return (
    <div className="flex flex-col gap-6">
      {/* 标题区 */}
      <div className="flex items-end justify-between">
        <div className="flex flex-col gap-2">
          <h1 className="text-[22px] font-bold tracking-tight text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px' }}>
            告警中心
          </h1>
          <p className="font-mono text-xs text-[#999999]">安全事件检测、告警规则管理与多渠道通知</p>
        </div>
        <div className="flex gap-2">
          <button onClick={() => setRulesListOpen(true)} className="flex items-center gap-2 border border-[#E8E8E8] bg-white px-4 py-2.5 font-mono text-[9px] font-semibold text-[#1A1A1A]">
            <Bell className="h-3 w-3 text-[#6a6a6a]" />
            告警规则
          </button>
          <button onClick={() => setRuleDialogOpen(true)} className="flex items-center gap-2 bg-[#0A6B3A] px-4 py-2.5 font-mono text-[9px] font-semibold text-white">
            <Plus className="h-3 w-3" />
            新建规则
          </button>
        </div>
      </div>

      {/* 统计卡片 */}
      <div className="grid grid-cols-4 gap-2.5">
        {statCards.map((s) => (
          <div key={s.label} className="flex flex-col gap-4 bg-white p-4" style={{ border: `1px solid ${s.borderColor}` }}>
            <span className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{s.label}</span>
            <div className="flex items-center gap-2">
              {s.dotColor && <div className="h-2 w-2 rounded-full" style={{ backgroundColor: s.dotColor }} />}
              <span className="text-[28px] font-bold tracking-tight" style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px', color: s.valueColor }}>
                {s.value}
              </span>
            </div>
          </div>
        ))}
      </div>

      {/* 筛选栏 */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="flex w-[220px] items-center gap-2.5 border border-[#E8E8E8] bg-white px-3.5 py-2">
            <Search className="h-3 w-3 text-[#6a6a6a]" />
            <input
              className="w-full bg-transparent font-mono text-[10px] font-medium text-[#1A1A1A] placeholder-[#CCCCCC] outline-none"
              placeholder="搜索告警..."
              value={search}
              onChange={(e) => { setSearch(e.target.value); setPage(1) }}
            />
          </div>
          <SelectFilter value={filterSeverity} onChange={(v) => { setFilterSeverity(v); setPage(1) }} placeholder="全部级别" options={severityOptions} />
          <SelectFilter value={filterStatus} onChange={(v) => { setFilterStatus(v); setPage(1) }} placeholder="全部状态" options={statusOptions} />
        </div>
        <span className="font-mono text-[10px] font-medium text-[#999999]">共 {total} 条告警</span>
      </div>

      {/* 表格 */}
      <div className="bg-white" style={{ border: '1px solid #E8E8E8' }}>
        <div className="grid grid-cols-6 px-4 py-2.5" style={{ borderBottom: '1px solid #E8E8E8' }}>
          {['时间', '级别', '规则名称', '触发详情', '状态', '操作'].map((h) => (
            <span key={h} className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{h}</span>
          ))}
        </div>

        {loading ? (
          <div className="flex items-center justify-center py-12">
            <Loader2 className="h-5 w-5 animate-spin text-[#999]" />
          </div>
        ) : events.length === 0 ? (
          <div className="flex items-center justify-center py-12">
            <span className="font-mono text-[10px] text-[#999]">暂无告警事件</span>
          </div>
        ) : (
          events.map((a, i) => {
            const sev = severityMap[a.severity] ?? { label: a.severity, color: '#999' }
            const st = statusMap[a.status] ?? { label: a.status, color: '#999' }
            const time = new Date(a.created_at).toLocaleTimeString('zh-CN', { hour12: false })
            return (
              <div key={a.id} className="grid grid-cols-6 items-center px-4 py-3" style={{ borderBottom: i < events.length - 1 ? '1px solid #E8E8E8' : 'none' }}>
                <span className="font-mono text-[10px] font-medium text-[#999999]">{time}</span>
                <div><StatusTag label={sev.label} color={sev.color} /></div>
                <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{a.rule_name}</span>
                <span className="truncate font-mono text-[10px] font-medium text-[#999999]">{a.trigger_detail}</span>
                <div><StatusTag label={st.label} color={st.color} /></div>
                <div>
                  <button
                    onClick={() => { setSelectedEvent(a); setStatusDialogOpen(true) }}
                    className="font-mono text-[10px] font-semibold text-[#0A6B3A]"
                  >
                    {a.status === 'pending' ? '处理' : '查看'}
                  </button>
                </div>
              </div>
            )
          })
        )}
        <TablePagination current={page} total={total} pageSize={pageSize} onChange={setPage} />
      </div>

      {/* 弹窗 */}
      <CreateRuleDialog open={ruleDialogOpen} onClose={() => setRuleDialogOpen(false)} onCreated={() => { setRuleDialogOpen(false); fetchStats() }} />
      <RulesListDialog open={rulesListOpen} onClose={() => setRulesListOpen(false)} />
      <UpdateStatusDialog open={statusDialogOpen} event={selectedEvent} onClose={() => { setStatusDialogOpen(false); setSelectedEvent(null) }} onSubmit={handleStatusUpdate} />
    </div>
  )
}

/* ── 通用筛选下拉组件 ── */

function SelectFilter({ value, onChange, placeholder, options }: {
  value: string; onChange: (v: string) => void
  placeholder: string; options: { value: string; label: string }[]
}) {
  return (
    <select
      value={value}
      onChange={(e) => onChange(e.target.value)}
      className="border border-[#E8E8E8] bg-white px-3.5 py-2 font-mono text-[10px] font-medium text-[#999999] outline-none"
    >
      <option value="">{placeholder}</option>
      {options.map((o) => (
        <option key={o.value} value={o.value}>{o.label}</option>
      ))}
    </select>
  )
}

const severityOptions = SEVERITIES.map((s) => ({ value: s, label: severityMap[s]?.label ?? s }))
const statusOptions = Object.entries(statusMap).map(([k, v]) => ({ value: k, label: v.label }))

/* ── 创建规则弹窗 ── */

function CreateRuleDialog({ open, onClose, onCreated }: { open: boolean; onClose: () => void; onCreated: () => void }) {
  const [form, setForm] = useState({
    name: '', description: '', event_type: 'dlp_violation', severity: 'high',
    notify_channels: [] as string[], silence_minutes: 60, enabled: true,
  })
  const [saving, setSaving] = useState(false)

  const toggleChannel = (ch: string) => {
    setForm((f) => ({
      ...f,
      notify_channels: f.notify_channels.includes(ch)
        ? f.notify_channels.filter((c) => c !== ch)
        : [...f.notify_channels, ch],
    }))
  }

  const handleSubmit = async () => {
    if (!form.name.trim()) return
    setSaving(true)
    try {
      await api.post('/alert-rules', form)
      setForm({ name: '', description: '', event_type: 'dlp_violation', severity: 'high', notify_channels: [], silence_minutes: 60, enabled: true })
      onCreated()
    } catch { /* 静默 */ }
    setSaving(false)
  }

  return (
    <Dialog open={open} onOpenChange={(v) => !v && onClose()}>
      <DialogContent className="max-w-[480px] p-0">
        <div className="flex items-center justify-between border-b border-[#E8E8E8] px-6 py-4">
          <span className="font-mono text-xs font-bold text-[#1A1A1A]">新建告警规则</span>
          <button onClick={onClose}><X className="h-4 w-4 text-[#999]" /></button>
        </div>
        <div className="flex flex-col gap-4 px-6 py-5">
          <div className="flex flex-col gap-1.5">
            <FormLabel required>规则名称</FormLabel>
            <input className={inputClass} value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} placeholder="如：DLP 拦截阈值超限" />
          </div>
          <div className="flex flex-col gap-1.5">
            <FormLabel>描述</FormLabel>
            <input className={inputClass} value={form.description} onChange={(e) => setForm({ ...form, description: e.target.value })} />
          </div>
          <div className="grid grid-cols-2 gap-4">
            <div className="flex flex-col gap-1.5">
              <FormLabel required>事件类型</FormLabel>
              <select className={inputClass} value={form.event_type} onChange={(e) => setForm({ ...form, event_type: e.target.value })}>
                {EVENT_TYPES.map((t) => <option key={t} value={t}>{eventTypeLabels[t]}</option>)}
              </select>
            </div>
            <div className="flex flex-col gap-1.5">
              <FormLabel required>严重级别</FormLabel>
              <select className={inputClass} value={form.severity} onChange={(e) => setForm({ ...form, severity: e.target.value })}>
                {SEVERITIES.map((s) => <option key={s} value={s}>{severityMap[s]?.label}</option>)}
              </select>
            </div>
          </div>
          <div className="flex flex-col gap-1.5">
            <FormLabel>通知渠道</FormLabel>
            <div className="flex gap-2">
              {CHANNELS.map((ch) => (
                <button key={ch} onClick={() => toggleChannel(ch)}
                  className={`border px-3 py-1.5 font-mono text-[9px] font-semibold ${form.notify_channels.includes(ch) ? 'border-[#0A6B3A] bg-[#0A6B3A1A] text-[#0A6B3A]' : 'border-[#E8E8E8] text-[#999]'}`}>
                  {ch}
                </button>
              ))}
            </div>
          </div>
          <div className="flex flex-col gap-1.5">
            <FormLabel>静默期（分钟）</FormLabel>
            <input type="number" className={`w-32 ${inputClass}`} value={form.silence_minutes} onChange={(e) => setForm({ ...form, silence_minutes: Number(e.target.value) })} />
          </div>
        </div>
        <div className="flex justify-end gap-2 border-t border-[#E8E8E8] px-6 py-4">
          <button onClick={onClose} className="border border-[#E8E8E8] px-4 py-2 font-mono text-[10px] font-semibold text-[#999]">取消</button>
          <button onClick={handleSubmit} disabled={saving || !form.name.trim()} className="bg-[#0A6B3A] px-4 py-2 font-mono text-[10px] font-semibold text-white disabled:opacity-50">
            {saving ? <Loader2 className="h-3 w-3 animate-spin" /> : '创建'}
          </button>
        </div>
      </DialogContent>
    </Dialog>
  )
}

/* ── 规则列表弹窗 ── */

function RulesListDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const [rules, setRules] = useState<AlertRule[]>([])
  const [loading, setLoading] = useState(false)

  useEffect(() => {
    if (!open) return
    setLoading(true)
    api.get('/alert-rules').then(({ data }) => setRules(data.rules)).catch(() => {}).finally(() => setLoading(false))
  }, [open])

  const toggleEnabled = async (rule: AlertRule) => {
    try {
      await api.put(`/alert-rules/${rule.id}`, { enabled: !rule.enabled })
      setRules((prev) => prev.map((r) => r.id === rule.id ? { ...r, enabled: !r.enabled } : r))
    } catch { /* 静默 */ }
  }

  const deleteRule = async (rule: AlertRule) => {
    if (!confirm(`确定删除规则「${rule.name}」？`)) return
    try {
      await api.delete(`/alert-rules/${rule.id}`)
      setRules((prev) => prev.filter((r) => r.id !== rule.id))
    } catch { /* 静默 */ }
  }

  return (
    <Dialog open={open} onOpenChange={(v) => !v && onClose()}>
      <DialogContent className="max-w-[600px] p-0">
        <div className="flex items-center justify-between border-b border-[#E8E8E8] px-6 py-4">
          <span className="font-mono text-xs font-bold text-[#1A1A1A]">告警规则管理</span>
          <button onClick={onClose}><X className="h-4 w-4 text-[#999]" /></button>
        </div>
        <div className="max-h-[400px] overflow-y-auto">
          {loading ? (
            <div className="flex items-center justify-center py-12"><Loader2 className="h-5 w-5 animate-spin text-[#999]" /></div>
          ) : rules.length === 0 ? (
            <div className="flex items-center justify-center py-12"><span className="font-mono text-[10px] text-[#999]">暂无告警规则</span></div>
          ) : (
            rules.map((rule) => {
              const sev = severityMap[rule.severity] ?? { label: rule.severity, color: '#999' }
              return (
                <div key={rule.id} className="flex items-center justify-between border-b border-[#E8E8E8] px-6 py-3">
                  <div className="flex flex-col gap-1">
                    <div className="flex items-center gap-2">
                      <span className="font-mono text-[10px] font-semibold text-[#1A1A1A]">{rule.name}</span>
                      <StatusTag label={sev.label} color={sev.color} />
                      <span className="font-mono text-[9px] text-[#999]">{eventTypeLabels[rule.event_type] ?? rule.event_type}</span>
                    </div>
                    {rule.description && <span className="font-mono text-[9px] text-[#999]">{rule.description}</span>}
                  </div>
                  <div className="flex items-center gap-3">
                    <button onClick={() => toggleEnabled(rule)} className={`font-mono text-[9px] font-semibold ${rule.enabled ? 'text-[#0A6B3A]' : 'text-[#999]'}`}>
                      {rule.enabled ? '已启用' : '已禁用'}
                    </button>
                    <button onClick={() => deleteRule(rule)} className="font-mono text-[9px] font-semibold text-[#CF1322]">删除</button>
                  </div>
                </div>
              )
            })
          )}
        </div>
      </DialogContent>
    </Dialog>
  )
}

/* ── 更新状态弹窗 ── */

function UpdateStatusDialog({ open, event, onClose, onSubmit }: {
  open: boolean; event: AlertEvent | null; onClose: () => void
  onSubmit: (status: string, note: string) => Promise<void>
}) {
  const [status, setStatus] = useState('acknowledged')
  const [note, setNote] = useState('')
  const [saving, setSaving] = useState(false)

  useEffect(() => {
    if (event) {
      setStatus(event.status === 'pending' ? 'acknowledged' : event.status)
      setNote(event.resolved_note ?? '')
    }
  }, [event])

  const handleSubmit = async () => {
    setSaving(true)
    await onSubmit(status, note)
    setSaving(false)
  }

  if (!event) return null

  const sev = severityMap[event.severity] ?? { label: event.severity, color: '#999' }

  return (
    <Dialog open={open} onOpenChange={(v) => !v && onClose()}>
      <DialogContent className="max-w-[440px] p-0">
        <div className="flex items-center justify-between border-b border-[#E8E8E8] px-6 py-4">
          <span className="font-mono text-xs font-bold text-[#1A1A1A]">处理告警</span>
          <button onClick={onClose}><X className="h-4 w-4 text-[#999]" /></button>
        </div>
        <div className="flex flex-col gap-4 px-6 py-5">
          <div className="flex items-center gap-2">
            <StatusTag label={sev.label} color={sev.color} />
            <span className="font-mono text-[10px] font-semibold text-[#1A1A1A]">{event.rule_name}</span>
          </div>
          <p className="font-mono text-[10px] text-[#999]">{event.trigger_detail}</p>
          <div className="flex flex-col gap-1.5">
            <FormLabel required>状态</FormLabel>
            <select className={inputClass} value={status} onChange={(e) => setStatus(e.target.value)}>
              {Object.entries(statusMap).map(([k, v]) => <option key={k} value={k}>{v.label}</option>)}
            </select>
          </div>
          <div className="flex flex-col gap-1.5">
            <FormLabel>处理备注</FormLabel>
            <textarea className={`h-20 resize-none ${inputClass}`} value={note} onChange={(e) => setNote(e.target.value)} placeholder="记录处理过程..." />
          </div>
        </div>
        <div className="flex justify-end gap-2 border-t border-[#E8E8E8] px-6 py-4">
          <button onClick={onClose} className="border border-[#E8E8E8] px-4 py-2 font-mono text-[10px] font-semibold text-[#999]">取消</button>
          <button onClick={handleSubmit} disabled={saving} className="bg-[#0A6B3A] px-4 py-2 font-mono text-[10px] font-semibold text-white disabled:opacity-50">
            {saving ? <Loader2 className="h-3 w-3 animate-spin" /> : '确认'}
          </button>
        </div>
      </DialogContent>
    </Dialog>
  )
}
