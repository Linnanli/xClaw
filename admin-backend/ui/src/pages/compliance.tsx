import { FileText, Loader2, X, BarChart2, Download } from 'lucide-react'
import { useState, useEffect, useCallback } from 'react'
import { api } from '@/lib/api'
import { TablePagination } from '@/components/ui/table-pagination'
import { StatusTag } from '@/components/ui/status-tag'
import { Dialog, DialogContent } from '@/components/ui/dialog'
import { FormLabel } from '@/components/ui/form-helpers'

const inputClass = 'border border-[#E8E8E8] px-3 py-2 font-mono text-[10px] outline-none'

const levelColors: Record<string, string> = {
  public: '#0A6B3A',
  internal: '#D48700',
  confidential: '#D4A200',
  top_secret: '#CF1322',
}

const typeMap: Record<string, { label: string; color: string }> = {
  monthly: { label: '月度报告', color: '#0A6B3A' },
  quarterly: { label: '季度报告', color: '#1677FF' },
  annual: { label: '年度报告', color: '#D48700' },
  custom: { label: '自定义', color: '#999999' },
}

interface LevelInfo { key: string; label: string }
interface Overview { levels: LevelInfo[]; rule_counts: Record<string, number>; total_rules: number; blocks_last_30d: number }
interface ReportContent { dlp_blocks: number; policy_changes: number; alert_events: number; approval_tickets: number }
interface Report { id: string; name: string; report_type: string; start_date: string; end_date: string; created_at: string; generated_by?: string; content?: ReportContent }
interface RetentionPolicy { classification_level: string; label: string; retention_days: number }

export default function CompliancePage() {
  const [overview, setOverview] = useState<Overview | null>(null)
  const [reports, setReports] = useState<Report[]>([])
  const [policies, setPolicies] = useState<RetentionPolicy[]>([])
  const [loading, setLoading] = useState(true)
  const [generateOpen, setGenerateOpen] = useState(false)
  const [detailReport, setDetailReport] = useState<Report | null>(null)
  const [page, setPage] = useState(1)

  const fetchAll = useCallback(async () => {
    setLoading(true)
    try {
      const [ovRes, rpRes, rtRes] = await Promise.all([
        api.get('/compliance/overview'),
        api.get('/compliance/reports'),
        api.get('/compliance/retention'),
      ])
      setOverview(ovRes.data)
      setReports(rpRes.data.reports)
      setPolicies(rtRes.data.policies)
    } catch { /* 静默 */ }
    setLoading(false)
  }, [])

  useEffect(() => { fetchAll() }, [fetchAll])

  const updateRetention = async (level: string, days: number) => {
    try {
      await api.put(`/compliance/retention/${level}`, { retention_days: days })
      fetchAll()
    } catch { /* 静默 */ }
  }

  return (
    <div className="flex flex-col gap-6">
      <div className="flex items-end justify-between">
        <div className="flex flex-col gap-2">
          <h1 className="text-[22px] font-bold tracking-tight text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px' }}>
            合规管理
          </h1>
          <p className="font-mono text-xs text-[#999999]">数据分类分级、合规报告与数据保留策略</p>
        </div>
        <button onClick={() => setGenerateOpen(true)} className="flex items-center gap-2 bg-[#0A6B3A] px-4 py-2.5 font-mono text-[9px] font-semibold text-white">
          <FileText className="h-3 w-3" />
          生成报告
        </button>
      </div>

      {/* 数据分级卡片 */}
      {loading ? (
        <div className="flex items-center justify-center py-12"><Loader2 className="h-5 w-5 animate-spin text-[#999]" /></div>
      ) : overview && (
        <div className="grid grid-cols-4 gap-2.5">
          {overview.levels.map((lv) => {
            const color = levelColors[lv.key] ?? '#999'
            const count = overview.rule_counts[lv.key] ?? 0
            return (
              <div key={lv.key} className="flex flex-col gap-3 bg-white p-4" style={{ border: `1px solid ${color}` }}>
                <span className="text-sm font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>{lv.label}</span>
                <div className="flex items-baseline gap-1">
                  <span className="text-[28px] font-bold tracking-tight" style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px', color }}>{count}</span>
                  <span className="font-mono text-[10px] font-medium text-[#999999]">规则</span>
                </div>
              </div>
            )
          })}
        </div>
      )}

      {/* 数据保留策略 */}
      {policies.length > 0 && (
        <div className="bg-white p-5" style={{ border: '1px solid #E8E8E8' }}>
          <span className="text-sm font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>数据保留策略</span>
          {/* TODO: 保留策略执行逻辑待实现 — 需明确各表数据与分级的映射关系后，添加后台定时清理任务 */}
          <div className="mt-3 flex flex-col gap-2">
            {policies.map((p) => (
              <div key={p.classification_level} className="flex items-center justify-between py-2" style={{ borderBottom: '1px solid #F0F0F0' }}>
                <span className="font-mono text-[10px] font-semibold" style={{ color: levelColors[p.classification_level] ?? '#999' }}>{p.label}</span>
                <div className="flex items-center gap-2">
                  <input type="number" defaultValue={p.retention_days} min={1}
                    className={`w-20 ${inputClass}`}
                    onBlur={(e) => {
                      const v = Number(e.target.value)
                      if (v > 0 && v !== p.retention_days) updateRetention(p.classification_level, v)
                    }}
                  />
                  <span className="font-mono text-[9px] text-[#999]">天</span>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* 合规报告表格 */}
      <div className="bg-white" style={{ border: '1px solid #E8E8E8' }}>
        <div className="px-4 py-3" style={{ borderBottom: '1px solid #E8E8E8' }}>
          <span className="text-sm font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>历史合规报告</span>
        </div>
        <div className="grid grid-cols-5 px-4 py-2.5" style={{ borderBottom: '1px solid #E8E8E8' }}>
          {['报告名称', '类型', '时间范围', '生成时间', '操作'].map((h) => (
            <span key={h} className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{h}</span>
          ))}
        </div>
        {reports.length === 0 ? (
          <div className="flex items-center justify-center py-12"><span className="font-mono text-[10px] text-[#999]">暂无合规报告</span></div>
        ) : (
          reports.map((r, i) => {
            const t = typeMap[r.report_type] ?? { label: r.report_type, color: '#999' }
            return (
              <div key={r.id} className="grid grid-cols-5 items-center px-4 py-3" style={{ borderBottom: i < reports.length - 1 ? '1px solid #E8E8E8' : 'none' }}>
                <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{r.name}</span>
                <div><StatusTag label={t.label} color={t.color} /></div>
                <span className="font-mono text-[10px] font-medium text-[#999999]">{r.start_date} ~ {r.end_date}</span>
                <span className="font-mono text-[10px] font-medium text-[#999999]">{new Date(r.created_at).toLocaleString('zh-CN')}</span>
                <button
                  onClick={() => setDetailReport(r)}
                  className="font-mono text-[10px] font-semibold text-[#0A6B3A] text-left hover:underline"
                >
                  查看
                </button>
              </div>
            )
          })
        )}
        <TablePagination current={page} total={reports.length} pageSize={10} onChange={setPage} />
      </div>

      <GenerateReportDialog open={generateOpen} onClose={() => setGenerateOpen(false)} onGenerated={() => { setGenerateOpen(false); fetchAll() }} />
      <ReportDetailDialog report={detailReport} onClose={() => setDetailReport(null)} />
    </div>
  )
}

// ============================================================================
// PDF 导出工具函数
// ============================================================================

const TYPE_LABELS: Record<string, string> = {
  monthly: '月度报告', quarterly: '季度报告', annual: '年度报告', custom: '自定义',
}

async function exportReportPdf(report: Report): Promise<void> {
  const { jsPDF } = await import('jspdf')
  const doc = new jsPDF({ orientation: 'portrait', unit: 'mm', format: 'a4' })

  const pageW = doc.internal.pageSize.getWidth()
  const margin = 20
  const contentW = pageW - margin * 2
  let y = margin

  // 标题区
  doc.setFontSize(18)
  doc.setFont('helvetica', 'bold')
  doc.text('IronClaw Compliance Report', margin, y)
  y += 10

  doc.setFontSize(10)
  doc.setFont('helvetica', 'normal')
  doc.setTextColor(100)
  doc.text(`Type: ${TYPE_LABELS[report.report_type] ?? report.report_type}`, margin, y)
  doc.text(`Period: ${report.start_date} ~ ${report.end_date}`, margin + contentW / 2, y)
  y += 6
  doc.text(`Generated: ${new Date(report.created_at).toLocaleString('zh-CN')}`, margin, y)
  if (report.generated_by) doc.text(`By: ${report.generated_by}`, margin + contentW / 2, y)
  y += 10

  // 分隔线
  doc.setDrawColor(200)
  doc.line(margin, y, pageW - margin, y)
  y += 8

  // 报告名称
  doc.setFontSize(14)
  doc.setFont('helvetica', 'bold')
  doc.setTextColor(26)
  doc.text(report.name, margin, y)
  y += 10

  // 统计数据
  if (report.content) {
    doc.setFontSize(11)
    doc.setFont('helvetica', 'bold')
    doc.text('Security Event Summary', margin, y)
    y += 7

    const stats = [
      { label: 'DLP Blocks (30d)', value: report.content.dlp_blocks },
      { label: 'Policy Changes', value: report.content.policy_changes },
      { label: 'Alert Events', value: report.content.alert_events },
      { label: 'Approval Tickets', value: report.content.approval_tickets },
    ]

    const colW = contentW / 2
    stats.forEach((s, i) => {
      const x = margin + (i % 2) * colW
      if (i % 2 === 0 && i > 0) y += 18

      doc.setFillColor(249, 249, 249)
      doc.rect(x, y, colW - 4, 16, 'F')
      doc.setFontSize(8)
      doc.setFont('helvetica', 'normal')
      doc.setTextColor(150)
      doc.text(s.label, x + 4, y + 5)
      doc.setFontSize(16)
      doc.setFont('helvetica', 'bold')
      doc.setTextColor(26)
      doc.text(String(s.value), x + 4, y + 13)
    })
    y += 22
  }

  // 页脚
  doc.setFontSize(8)
  doc.setFont('helvetica', 'normal')
  doc.setTextColor(150)
  doc.text('Generated by IronClaw Admin Platform', margin, doc.internal.pageSize.getHeight() - 10)

  doc.save(`compliance-report-${report.id.slice(0, 8)}.pdf`)
}

// ============================================================================
// 报告详情弹窗
// ============================================================================

function ReportDetailDialog({ report, onClose }: { report: Report | null; onClose: () => void }) {
  const [exporting, setExporting] = useState(false)

  if (!report) return null
  const currentReport = report

  const t = typeMap[currentReport.report_type] ?? { label: currentReport.report_type, color: '#999' }
  const c = currentReport.content

  const stats = c ? [
    { label: 'DLP 拦截次数', value: c.dlp_blocks, color: '#CF1322' },
    { label: '策略变更次数', value: c.policy_changes, color: '#D48700' },
    { label: '告警事件数', value: c.alert_events, color: '#1677FF' },
    { label: '审批工单数', value: c.approval_tickets, color: '#0A6B3A' },
  ] : []

  async function handleExport() {
    setExporting(true)
    try {
      await exportReportPdf(currentReport)
    } finally {
      setExporting(false)
    }
  }

  return (
    <Dialog open={!!report} onOpenChange={(v) => !v && onClose()}>
      <DialogContent className="max-w-[520px] p-0">
        <div className="flex items-center justify-between border-b border-[#E8E8E8] px-6 py-4">
          <div className="flex flex-col gap-0.5">
            <span className="font-mono text-xs font-bold text-[#1A1A1A]">{report.name}</span>
            <span className="font-mono text-[9px] text-[#999]">{report.start_date} ~ {report.end_date}</span>
          </div>
          <div className="flex items-center gap-3">
            <StatusTag label={t.label} color={t.color} />
            <button onClick={onClose}><X className="h-4 w-4 text-[#999]" /></button>
          </div>
        </div>

        <div className="px-6 py-5">
          {c ? (
            <>
              <div className="mb-4 flex items-center gap-1.5">
                <BarChart2 className="h-3.5 w-3.5 text-[#0A6B3A]" />
                <span className="font-mono text-[10px] font-semibold text-[#1A1A1A]">安全事件汇总</span>
              </div>
              <div className="grid grid-cols-2 gap-3">
                {stats.map((s) => (
                  <div key={s.label} className="flex flex-col gap-2 bg-[#F9F9F9] p-3" style={{ border: '1px solid #F0F0F0' }}>
                    <span className="font-mono text-[9px] text-[#999]">{s.label}</span>
                    <span className="text-[24px] font-bold tracking-tight" style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px', color: s.color }}>
                      {s.value}
                    </span>
                  </div>
                ))}
              </div>
            </>
          ) : (
            <div className="flex items-center justify-center py-8">
              <span className="font-mono text-[10px] text-[#999]">报告内容暂不可用</span>
            </div>
          )}

          <div className="mt-4 flex flex-col gap-1.5 border-t border-[#F0F0F0] pt-4">
            <div className="flex justify-between">
              <span className="font-mono text-[9px] text-[#999]">生成时间</span>
              <span className="font-mono text-[9px] text-[#1A1A1A]">{new Date(report.created_at).toLocaleString('zh-CN')}</span>
            </div>
            {report.generated_by && (
              <div className="flex justify-between">
                <span className="font-mono text-[9px] text-[#999]">生成人</span>
                <span className="font-mono text-[9px] text-[#1A1A1A]">{report.generated_by}</span>
              </div>
            )}
          </div>
        </div>

        <div className="flex justify-between border-t border-[#E8E8E8] px-6 py-4">
          <button
            onClick={handleExport}
            disabled={exporting}
            className="flex items-center gap-1.5 border border-[#0A6B3A] px-4 py-2 font-mono text-[10px] font-semibold text-[#0A6B3A] disabled:opacity-50"
          >
            {exporting ? <Loader2 className="h-3 w-3 animate-spin" /> : <Download className="h-3 w-3" />}
            导出 PDF
          </button>
          <button onClick={onClose} className="border border-[#E8E8E8] px-4 py-2 font-mono text-[10px] font-semibold text-[#999]">关闭</button>
        </div>
      </DialogContent>
    </Dialog>
  )
}

// ============================================================================
// 生成报告弹窗
// ============================================================================

function GenerateReportDialog({ open, onClose, onGenerated }: { open: boolean; onClose: () => void; onGenerated: () => void }) {
  const [form, setForm] = useState({ name: '', report_type: 'monthly', start_date: '', end_date: '' })
  const [saving, setSaving] = useState(false)

  const handleSubmit = async () => {
    if (!form.name.trim() || !form.start_date || !form.end_date) return
    setSaving(true)
    try {
      await api.post('/compliance/reports', form)
      setForm({ name: '', report_type: 'monthly', start_date: '', end_date: '' })
      onGenerated()
    } catch { /* 静默 */ }
    setSaving(false)
  }

  return (
    <Dialog open={open} onOpenChange={(v) => !v && onClose()}>
      <DialogContent className="max-w-[440px] p-0">
        <div className="flex items-center justify-between border-b border-[#E8E8E8] px-6 py-4">
          <span className="font-mono text-xs font-bold text-[#1A1A1A]">生成合规报告</span>
          <button onClick={onClose}><X className="h-4 w-4 text-[#999]" /></button>
        </div>
        <div className="flex flex-col gap-4 px-6 py-5">
          <div className="flex flex-col gap-1.5">
            <FormLabel required>报告名称</FormLabel>
            <input className={inputClass} value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} placeholder="如：2024年Q1合规报告" />
          </div>
          <div className="flex flex-col gap-1.5">
            <FormLabel required>报告类型</FormLabel>
            <select className={inputClass} value={form.report_type} onChange={(e) => setForm({ ...form, report_type: e.target.value })}>
              {Object.entries(typeMap).map(([k, v]) => <option key={k} value={k}>{v.label}</option>)}
            </select>
          </div>
          <div className="grid grid-cols-2 gap-4">
            <div className="flex flex-col gap-1.5">
              <FormLabel required>开始日期</FormLabel>
              <input type="date" className={inputClass} value={form.start_date} onChange={(e) => setForm({ ...form, start_date: e.target.value })} />
            </div>
            <div className="flex flex-col gap-1.5">
              <FormLabel required>结束日期</FormLabel>
              <input type="date" className={inputClass} value={form.end_date} onChange={(e) => setForm({ ...form, end_date: e.target.value })} />
            </div>
          </div>
        </div>
        <div className="flex justify-end gap-2 border-t border-[#E8E8E8] px-6 py-4">
          <button onClick={onClose} className="border border-[#E8E8E8] px-4 py-2 font-mono text-[10px] font-semibold text-[#999]">取消</button>
          <button onClick={handleSubmit} disabled={saving || !form.name.trim() || !form.start_date || !form.end_date}
            className="bg-[#0A6B3A] px-4 py-2 font-mono text-[10px] font-semibold text-white disabled:opacity-50">
            {saving ? <Loader2 className="h-3 w-3 animate-spin" /> : '生成'}
          </button>
        </div>
      </DialogContent>
    </Dialog>
  )
}
