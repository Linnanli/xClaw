import { Search, Download, Loader2, X, MessageSquare, ImageIcon, FileText, AudioLines } from 'lucide-react'
import { useState, useEffect, useCallback } from 'react'
import { api } from '@/lib/api'
import { TablePagination } from '@/components/ui/table-pagination'
import { StatusTag } from '@/components/ui/status-tag'
import { Dialog, DialogContent } from '@/components/ui/dialog'
import type { Conversation, ConversationAttachment, ConversationDetail, ConversationStats } from '@/types'

function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`
  return String(n)
}

function formatBytes(n?: number): string {
  if (!n) return '未知大小'
  if (n >= 1_048_576) return `${(n / 1_048_576).toFixed(1)} MB`
  if (n >= 1024) return `${(n / 1024).toFixed(1)} KB`
  return `${n} B`
}

function attachmentPreviewSrc(attachment: ConversationAttachment): string | null {
  if (!attachment.image_data_base64) return null
  return `data:${attachment.mime_type};base64,${attachment.image_data_base64}`
}

function attachmentIcon(attachment: ConversationAttachment) {
  if (attachment.kind === 'image') return <ImageIcon className="h-3.5 w-3.5 text-[#1677FF]" />
  if (attachment.kind === 'audio') return <AudioLines className="h-3.5 w-3.5 text-[#7A4D00]" />
  return <FileText className="h-3.5 w-3.5 text-[#5B5B5B]" />
}

function attachmentLabel(attachment: ConversationAttachment): string {
  if (attachment.filename) return attachment.filename
  if (attachment.kind === 'image') return '图片附件'
  if (attachment.kind === 'audio') return '音频附件'
  return '文档附件'
}

export default function ConversationsPage() {
  const [conversations, setConversations] = useState<Conversation[]>([])
  const [stats, setStats] = useState<ConversationStats>({ today_count: 0, today_tokens: 0, today_dlp_flagged: 0, today_active_users: 0 })
  const [total, setTotal] = useState(0)
  const [page, setPage] = useState(1)
  const [loading, setLoading] = useState(true)
  const [search, setSearch] = useState('')
  const [dlpFilter, setDlpFilter] = useState('')
  const [detailOpen, setDetailOpen] = useState(false)
  const [detail, setDetail] = useState<ConversationDetail | null>(null)
  const [detailLoading, setDetailLoading] = useState(false)
  const pageSize = 20

  const fetchList = useCallback(async () => {
    setLoading(true)
    try {
      const params = new URLSearchParams({ page: String(page), page_size: String(pageSize) })
      if (search.trim()) params.set('search', search.trim())
      if (dlpFilter === 'flagged') params.set('dlp_flagged', 'true')
      if (dlpFilter === 'clean') params.set('dlp_flagged', 'false')
      const { data } = await api.get(`/conversations?${params}`)
      setConversations(data.data)
      setTotal(data.total)
    } catch { /* 静默 */ }
    setLoading(false)
  }, [page, search, dlpFilter])

  const fetchStats = useCallback(async () => {
    try {
      const { data } = await api.get('/conversations/stats')
      setStats(data)
    } catch { /* 静默 */ }
  }, [])

  useEffect(() => { fetchList() }, [fetchList])
  useEffect(() => { fetchStats() }, [fetchStats])

  const openDetail = async (id: string) => {
    setDetailOpen(true)
    setDetailLoading(true)
    try {
      const { data } = await api.get(`/conversations/${id}`)
      setDetail(data)
    } catch { setDetail(null) }
    setDetailLoading(false)
  }

  const statCards = [
    { label: '今日对话', value: String(stats.today_count), dotColor: '', valueColor: '#1A1A1A', borderColor: '#E8E8E8' },
    { label: 'Token 消耗', value: formatTokens(stats.today_tokens), dotColor: '', valueColor: '#0A6B3A', borderColor: '#E8E8E8' },
    { label: 'DLP 标记', value: String(stats.today_dlp_flagged), dotColor: stats.today_dlp_flagged > 0 ? '#CF1322' : '', valueColor: stats.today_dlp_flagged > 0 ? '#CF1322' : '#1A1A1A', borderColor: stats.today_dlp_flagged > 0 ? 'rgba(207,19,34,0.25)' : '#E8E8E8' },
    { label: '活跃用户', value: String(stats.today_active_users), dotColor: '', valueColor: '#1A1A1A', borderColor: '#E8E8E8' },
  ]

  return (
    <div className="flex flex-col gap-6">
      {/* 标题区 */}
      <div className="flex items-end justify-between">
        <div className="flex flex-col gap-2">
          <h1 className="text-[22px] font-bold tracking-tight text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px' }}>
            对话审计
          </h1>
          <p className="font-mono text-xs text-[#999999]">审计员工与 AI 助手的对话记录，确保合规使用</p>
        </div>
        <button className="flex items-center gap-2 border border-[#E8E8E8] bg-white px-4 py-2.5 font-mono text-[9px] font-semibold text-[#1A1A1A]">
          <Download className="h-3 w-3 text-[#6a6a6a]" />
          导出报告
        </button>
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
            <input className="w-full bg-transparent font-mono text-[10px] font-medium text-[#1A1A1A] placeholder-[#CCCCCC] outline-none" placeholder="搜索用户或主题..." value={search} onChange={(e) => { setSearch(e.target.value); setPage(1) }} />
          </div>
          <select value={dlpFilter} onChange={(e) => { setDlpFilter(e.target.value); setPage(1) }} className="border border-[#E8E8E8] bg-white px-3.5 py-2 font-mono text-[10px] font-medium text-[#999999] outline-none">
            <option value="">全部 DLP</option>
            <option value="flagged">已标记</option>
            <option value="clean">未标记</option>
          </select>
        </div>
        <span className="font-mono text-[10px] font-medium text-[#999999]">共 {total} 条对话</span>
      </div>

      {/* 表格 */}
      <div className="bg-white" style={{ border: '1px solid #E8E8E8' }}>
        <div className="grid grid-cols-7 px-4 py-2.5" style={{ borderBottom: '1px solid #E8E8E8' }}>
          {['用户', '对话主题', '消息数', 'TOKEN', '模型', 'DLP', '时间'].map((h) => (
            <span key={h} className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{h}</span>
          ))}
        </div>

        {loading ? (
          <div className="flex items-center justify-center py-12"><Loader2 className="h-5 w-5 animate-spin text-[#999]" /></div>
        ) : conversations.length === 0 ? (
          <div className="flex items-center justify-center py-12"><span className="font-mono text-[10px] text-[#999]">暂无对话记录</span></div>
        ) : (
          conversations.map((c, i) => {
            const time = new Date(c.created_at).toLocaleTimeString('zh-CN', { hour12: false })
            return (
              <div key={c.id} className="grid cursor-pointer grid-cols-7 items-center px-4 py-3 hover:bg-[#FAFAFA]" style={{ borderBottom: i < conversations.length - 1 ? '1px solid #E8E8E8' : 'none' }} onClick={() => openDetail(c.id)}>
                <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{c.username}</span>
                <span className="truncate font-mono text-[10px] font-medium text-[#1A1A1A]">{c.topic || '—'}</span>
                <span className="font-mono text-[10px] font-medium text-[#999999]">{c.message_count}</span>
                <span className="font-mono text-[10px] font-medium text-[#999999]">{formatTokens(c.total_tokens)}</span>
                <div>
                  {c.model_id ? (
                    <StatusTag label={c.model_id} color="#1677FF" />
                  ) : (
                    <span className="font-mono text-[10px] text-[#999]">—</span>
                  )}
                </div>
                <div>
                  {c.dlp_flagged ? (
                    <StatusTag label="标记" color="#CF1322" />
                  ) : (
                    <span className="font-mono text-[10px] text-[#999]">—</span>
                  )}
                </div>
                <span className="font-mono text-[10px] font-medium text-[#999999]">{time}</span>
              </div>
            )
          })
        )}
        <TablePagination current={page} total={total} pageSize={pageSize} onChange={setPage} />
      </div>

      {/* 对话详情弹窗 */}
      <ConversationDetailDialog open={detailOpen} detail={detail} loading={detailLoading} onClose={() => { setDetailOpen(false); setDetail(null) }} />
    </div>
  )
}

/* ── 对话详情弹窗 ── */

function ConversationDetailDialog({ open, detail, loading, onClose }: {
  open: boolean; detail: ConversationDetail | null; loading: boolean; onClose: () => void
}) {
  return (
    <Dialog open={open} onOpenChange={(v) => !v && onClose()}>
      <DialogContent className="max-w-[640px] p-0">
        <div className="flex items-center justify-between border-b border-[#E8E8E8] px-6 py-4">
          <div className="flex items-center gap-2">
            <MessageSquare className="h-4 w-4 text-[#0A6B3A]" />
            <span className="font-mono text-xs font-bold text-[#1A1A1A]">对话详情</span>
          </div>
          <button onClick={onClose}><X className="h-4 w-4 text-[#999]" /></button>
        </div>

        {loading ? (
          <div className="flex items-center justify-center py-16"><Loader2 className="h-5 w-5 animate-spin text-[#999]" /></div>
        ) : !detail ? (
          <div className="flex items-center justify-center py-16"><span className="font-mono text-[10px] text-[#999]">加载失败</span></div>
        ) : (
          <>
            {/* 摘要信息 */}
            <div className="flex flex-wrap gap-4 border-b border-[#E8E8E8] px-6 py-3">
              <InfoItem label="用户" value={detail.username} />
              <InfoItem label="主题" value={detail.topic || '—'} />
              <InfoItem label="消息数" value={String(detail.message_count)} />
              <InfoItem label="Token" value={formatTokens(detail.total_tokens)} />
              {detail.model_id && <InfoItem label="模型" value={detail.model_id} />}
              {detail.dlp_flagged && <InfoItem label="DLP" value={detail.dlp_details || '已标记'} color="#CF1322" />}
            </div>

            {/* 消息流 */}
            <div className="max-h-[400px] overflow-y-auto px-6 py-4">
              <div className="flex flex-col gap-3">
                {detail.messages.map((msg) => (
                  <div key={msg.id} className={`flex flex-col gap-1 rounded px-3 py-2 ${msg.role === 'user' ? 'bg-[#F5F5F5]' : 'bg-[#0A6B3A08]'}`}>
                    <div className="flex items-center justify-between">
                      <span className="font-mono text-[9px] font-semibold" style={{ color: msg.role === 'user' ? '#1A1A1A' : '#0A6B3A' }}>
                        {msg.role === 'user' ? '用户' : msg.role === 'assistant' ? 'AI' : '系统'}
                      </span>
                      {(msg.input_tokens > 0 || msg.output_tokens > 0) && (
                        <span className="font-mono text-[8px] text-[#999]">
                          {msg.input_tokens > 0 ? `入${msg.input_tokens}` : ''}{msg.output_tokens > 0 ? ` 出${msg.output_tokens}` : ''}
                        </span>
                      )}
                    </div>
                    <p className="whitespace-pre-wrap font-mono text-[10px] text-[#333]">{msg.content}</p>
                    {msg.attachments.length > 0 && (
                      <ConversationAttachments attachments={msg.attachments} />
                    )}
                  </div>
                ))}
              </div>
            </div>
          </>
        )}
      </DialogContent>
    </Dialog>
  )
}

function InfoItem({ label, value, color }: { label: string; value: string; color?: string }) {
  return (
    <div className="flex items-center gap-1.5">
      <span className="font-mono text-[9px] text-[#999]">{label}:</span>
      <span className="font-mono text-[10px] font-medium" style={{ color: color || '#1A1A1A' }}>{value}</span>
    </div>
  )
}

function ConversationAttachments({ attachments }: { attachments: ConversationAttachment[] }) {
  return (
    <div className="mt-1 flex flex-col gap-2 border-t border-[#E8E8E8] pt-2">
      <span className="font-mono text-[8px] font-semibold tracking-[0.4px] text-[#999999]">附件</span>
      <div className="flex flex-col gap-2">
        {attachments.map((attachment) => {
          const previewSrc = attachmentPreviewSrc(attachment)
          return (
            <div key={attachment.id} className="rounded border border-[#E8E8E8] bg-white px-2.5 py-2">
              <div className="flex items-center gap-2">
                {attachmentIcon(attachment)}
                <div className="flex min-w-0 flex-1 flex-col gap-0.5">
                  <span className="truncate font-mono text-[10px] font-medium text-[#1A1A1A]">{attachmentLabel(attachment)}</span>
                  <span className="font-mono text-[8px] text-[#999999]">
                    {attachment.mime_type} · {formatBytes(attachment.size_bytes)}
                  </span>
                </div>
              </div>
              {previewSrc && (
                <img
                  src={previewSrc}
                  alt={attachmentLabel(attachment)}
                  className="mt-2 max-h-[180px] rounded border border-[#E8E8E8] object-contain"
                />
              )}
              {attachment.extracted_text && (
                <div className="mt-2 rounded bg-[#FAFAFA] px-2 py-1.5">
                  <span className="font-mono text-[8px] leading-4 text-[#666666]">{attachment.extracted_text}</span>
                </div>
              )}
            </div>
          )
        })}
      </div>
    </div>
  )
}
