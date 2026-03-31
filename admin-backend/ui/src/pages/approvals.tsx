import { Loader2, X } from 'lucide-react'
import { useState, useEffect, useCallback } from 'react'
import { api } from '@/lib/api'
import { TablePagination } from '@/components/ui/table-pagination'
import { StatusTag } from '@/components/ui/status-tag'
import { Dialog, DialogContent } from '@/components/ui/dialog'
import { FormLabel } from '@/components/ui/form-helpers'
import type { ApprovalTicket, ApprovalStats } from '@/types'

const statusMap: Record<string, { label: string; color: string }> = {
  pending: { label: '待审批', color: '#CF1322' },
  approved: { label: '已批准', color: '#0A6B3A' },
  rejected: { label: '已拒绝', color: '#999999' },
  expired: { label: '已过期', color: '#D48700' },
}

const inputClass = 'border border-[#E8E8E8] px-3 py-2 font-mono text-[10px] outline-none'

export default function ApprovalsPage() {
  const [tickets, setTickets] = useState<ApprovalTicket[]>([])
  const [stats, setStats] = useState<ApprovalStats>({ pending: 0, approved: 0, rejected: 0, expired: 0 })
  const [total, setTotal] = useState(0)
  const [page, setPage] = useState(1)
  const [loading, setLoading] = useState(true)
  const [filterStatus, setFilterStatus] = useState('')
  const [reviewOpen, setReviewOpen] = useState(false)
  const [selectedTicket, setSelectedTicket] = useState<ApprovalTicket | null>(null)
  const pageSize = 20

  const fetchList = useCallback(async () => {
    setLoading(true)
    try {
      const params = new URLSearchParams({ page: String(page), page_size: String(pageSize) })
      if (filterStatus) params.set('status', filterStatus)
      const { data } = await api.get(`/approvals?${params}`)
      setTickets(data.data)
      setTotal(data.total)
    } catch { /* 静默 */ }
    setLoading(false)
  }, [page, filterStatus])

  const fetchStats = useCallback(async () => {
    try {
      const { data } = await api.get('/approvals/stats')
      setStats(data)
    } catch { /* 静默 */ }
  }, [])

  useEffect(() => { fetchList() }, [fetchList])
  useEffect(() => { fetchStats() }, [fetchStats])

  const handleReview = async (action: string, comment: string) => {
    if (!selectedTicket) return
    try {
      await api.put(`/approvals/${selectedTicket.id}/review`, { action, comment: comment || undefined })
      setReviewOpen(false)
      setSelectedTicket(null)
      fetchList()
      fetchStats()
    } catch { /* 静默 */ }
  }

  const statCards = [
    { label: '待审批', value: stats.pending, dotColor: '#CF1322', valueColor: '#CF1322', borderColor: 'rgba(207,19,34,0.25)' },
    { label: '已批准', value: stats.approved, dotColor: '', valueColor: '#0A6B3A', borderColor: '#E8E8E8' },
    { label: '已拒绝', value: stats.rejected, dotColor: '', valueColor: '#999999', borderColor: '#E8E8E8' },
    { label: '已过期', value: stats.expired, dotColor: '', valueColor: '#D48700', borderColor: 'rgba(212,135,0,0.25)' },
  ]

  return (
    <div className="flex flex-col gap-6">
      <div className="flex items-end justify-between">
        <div className="flex flex-col gap-2">
          <h1 className="text-[22px] font-bold tracking-tight text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px' }}>
            审批工单
          </h1>
          <p className="font-mono text-xs text-[#999999]">高风险操作审批流程管理</p>
        </div>
      </div>

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

      <div className="flex items-center justify-between">
        <select value={filterStatus} onChange={(e) => { setFilterStatus(e.target.value); setPage(1) }}
          className="border border-[#E8E8E8] bg-white px-3.5 py-2 font-mono text-[10px] font-medium text-[#999999] outline-none">
          <option value="">全部状态</option>
          {Object.entries(statusMap).map(([k, v]) => <option key={k} value={k}>{v.label}</option>)}
        </select>
        <span className="font-mono text-[10px] font-medium text-[#999999]">共 {total} 条工单</span>
      </div>

      <div className="bg-white" style={{ border: '1px solid #E8E8E8' }}>
        <div className="grid grid-cols-6 px-4 py-2.5" style={{ borderBottom: '1px solid #E8E8E8' }}>
          {['申请人', '操作类型', '申请时间', '过期时间', '状态', '操作'].map((h) => (
            <span key={h} className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{h}</span>
          ))}
        </div>

        {loading ? (
          <div className="flex items-center justify-center py-12"><Loader2 className="h-5 w-5 animate-spin text-[#999]" /></div>
        ) : tickets.length === 0 ? (
          <div className="flex items-center justify-center py-12"><span className="font-mono text-[10px] text-[#999]">暂无审批工单</span></div>
        ) : (
          tickets.map((t, i) => {
            const st = statusMap[t.status] ?? { label: t.status, color: '#999' }
            const applyTime = new Date(t.created_at).toLocaleString('zh-CN', { month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' })
            const expireTime = new Date(t.expires_at).toLocaleString('zh-CN', { month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' })
            const isExpireSoon = t.status === 'pending' && new Date(t.expires_at).getTime() - Date.now() < 4 * 3600 * 1000
            return (
              <div key={t.id} className="grid grid-cols-6 items-center px-4 py-3" style={{ borderBottom: i < tickets.length - 1 ? '1px solid #E8E8E8' : 'none' }}>
                <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{t.applicant}</span>
                <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{t.operation_name}</span>
                <span className="font-mono text-[10px] font-medium text-[#999999]">{applyTime}</span>
                <span className="font-mono text-[10px] font-medium" style={{ color: isExpireSoon ? '#D48700' : '#999999' }}>{expireTime}</span>
                <div><StatusTag label={st.label} color={st.color} /></div>
                <div className="flex items-center gap-2">
                  {t.status === 'pending' ? (
                    <button onClick={() => { setSelectedTicket(t); setReviewOpen(true) }} className="font-mono text-[10px] font-semibold text-[#0A6B3A]">审批</button>
                  ) : (
                    <button onClick={() => { setSelectedTicket(t); setReviewOpen(true) }} className="font-mono text-[10px] font-semibold text-[#0A6B3A]">查看</button>
                  )}
                </div>
              </div>
            )
          })
        )}
        <TablePagination current={page} total={total} pageSize={pageSize} onChange={setPage} />
      </div>

      <ReviewDialog open={reviewOpen} ticket={selectedTicket} onClose={() => { setReviewOpen(false); setSelectedTicket(null) }} onSubmit={handleReview} />
    </div>
  )
}

function ReviewDialog({ open, ticket, onClose, onSubmit }: {
  open: boolean; ticket: ApprovalTicket | null; onClose: () => void
  onSubmit: (action: string, comment: string) => Promise<void>
}) {
  const [comment, setComment] = useState('')
  const [saving, setSaving] = useState(false)
  const isPending = ticket?.status === 'pending'

  useEffect(() => {
    if (ticket) setComment(ticket.review_comment ?? '')
  }, [ticket])

  const handleAction = async (action: string) => {
    setSaving(true)
    await onSubmit(action, comment)
    setSaving(false)
  }

  if (!ticket) return null
  const st = statusMap[ticket.status] ?? { label: ticket.status, color: '#999' }

  return (
    <Dialog open={open} onOpenChange={(v) => !v && onClose()}>
      <DialogContent className="max-w-[440px] p-0">
        <div className="flex items-center justify-between border-b border-[#E8E8E8] px-6 py-4">
          <span className="font-mono text-xs font-bold text-[#1A1A1A]">{isPending ? '审批工单' : '工单详情'}</span>
          <button onClick={onClose}><X className="h-4 w-4 text-[#999]" /></button>
        </div>
        <div className="flex flex-col gap-4 px-6 py-5">
          <div className="flex items-center gap-2">
            <StatusTag label={st.label} color={st.color} />
            <span className="font-mono text-[10px] font-semibold text-[#1A1A1A]">{ticket.operation_name}</span>
          </div>
          <div className="flex flex-col gap-1">
            <span className="font-mono text-[9px] text-[#999]">申请人: {ticket.applicant}</span>
            <span className="font-mono text-[9px] text-[#999]">操作类型: {ticket.operation_type}</span>
            {ticket.reason && <span className="font-mono text-[9px] text-[#999]">申请理由: {ticket.reason}</span>}
          </div>
          {isPending && (
            <div className="flex flex-col gap-1.5">
              <FormLabel>审批意见</FormLabel>
              <textarea className={`h-20 resize-none ${inputClass}`} value={comment} onChange={(e) => setComment(e.target.value)} placeholder="填写审批意见..." />
            </div>
          )}
          {!isPending && ticket.review_comment && (
            <div className="flex flex-col gap-1">
              <span className="font-mono text-[9px] font-semibold text-[#999]">审批意见</span>
              <p className="font-mono text-[10px] text-[#333]">{ticket.review_comment}</p>
            </div>
          )}
        </div>
        {isPending && (
          <div className="flex justify-end gap-2 border-t border-[#E8E8E8] px-6 py-4">
            <button onClick={() => handleAction('reject')} disabled={saving}
              className="border border-[#CF1322] px-4 py-2 font-mono text-[10px] font-semibold text-[#CF1322] disabled:opacity-50">
              拒绝
            </button>
            <button onClick={() => handleAction('approve')} disabled={saving}
              className="bg-[#0A6B3A] px-4 py-2 font-mono text-[10px] font-semibold text-white disabled:opacity-50">
              {saving ? <Loader2 className="h-3 w-3 animate-spin" /> : '批准'}
            </button>
          </div>
        )}
      </DialogContent>
    </Dialog>
  )
}
