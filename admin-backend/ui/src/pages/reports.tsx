import { FileText, Download, RefreshCw, Loader2 } from 'lucide-react'
import { useState, useEffect, useCallback } from 'react'
import { api } from '@/lib/api'

/* ── 类型 ── */

interface ReportStats {
  dlpRuleCount: number
  sensitiveOpsCount: number
  dictionaryCount: number
  totalKeywords: number
  clientTotal: number
}

/* ── 组件 ── */

export default function ReportsPage() {
  const [stats, setStats] = useState<ReportStats | null>(null)
  const [loading, setLoading] = useState(true)

  const loadReport = useCallback(async () => {
    setLoading(true)
    try {
      const [rulesRes, opsRes, dictsRes, clientsRes] = await Promise.allSettled([
        api.get('/dlp-rules'),
        api.get('/sensitive-operations'),
        api.get('/dlp-dictionaries'),
        api.get('/clients/stats'),
      ])

      const rules = rulesRes.status === 'fulfilled' ? (rulesRes.value.data.rules || []) : []
      const ops = opsRes.status === 'fulfilled' ? (opsRes.value.data.operations || []) : []
      const dicts = dictsRes.status === 'fulfilled' ? (dictsRes.value.data.dictionaries || []) : []
      const clientStats = clientsRes.status === 'fulfilled' ? clientsRes.value.data : { total: 0 }

      const totalKeywords = dicts.reduce((sum: number, d: { keyword_count?: number }) => sum + (d.keyword_count || 0), 0)

      setStats({
        dlpRuleCount: rules.length,
        sensitiveOpsCount: ops.length,
        dictionaryCount: dicts.length,
        totalKeywords,
        clientTotal: clientStats.total || 0,
      })
    } catch (err) {
      console.error('加载报表数据失败', err)
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => { loadReport() }, [loadReport])

  const reportCards = [
    { label: 'DLP 规则数', value: stats ? String(stats.dlpRuleCount) : '—', valueColor: '#1A1A1A', borderColor: '#E8E8E8' },
    { label: '敏感操作数', value: stats ? String(stats.sensitiveOpsCount) : '—', valueColor: '#CF1322', borderColor: 'rgba(207,19,34,0.25)' },
    { label: '字典数 / 关键字', value: stats ? `${stats.dictionaryCount} / ${stats.totalKeywords}` : '—', valueColor: '#1A1A1A', borderColor: '#E8E8E8' },
    { label: '客户端数', value: stats ? String(stats.clientTotal) : '—', valueColor: '#D48700', borderColor: 'rgba(212,135,0,0.25)' },
  ]

  return (
    <div className="flex flex-col gap-6">

      {/* 标题区 */}
      <div className="flex items-end justify-between">
        <div className="flex flex-col gap-2">
          <h1 className="text-[22px] font-bold tracking-tight text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px' }}>
            统计报表
          </h1>
          <p className="font-mono text-xs text-[#999999]">DLP、安全事件、AI 使用量与费用统计</p>
        </div>
        <div className="flex gap-2">
          <button
            className="flex items-center gap-2 border border-[#E8E8E8] bg-white px-4 py-2.5 font-mono text-[9px] font-semibold text-[#1A1A1A]"
            onClick={loadReport}
          >
            {loading ? <Loader2 className="h-3 w-3 animate-spin text-[#6a6a6a]" /> : <RefreshCw className="h-3 w-3 text-[#6a6a6a]" />}
            刷新
          </button>
          <button className="flex items-center gap-2 bg-[#0A6B3A] px-4 py-2.5 font-mono text-[9px] font-semibold text-white">
            <FileText className="h-3 w-3" />
            导出 PDF
          </button>
          <button className="flex items-center gap-2 border border-[#E8E8E8] bg-white px-4 py-2.5 font-mono text-[9px] font-semibold text-[#1A1A1A]">
            <Download className="h-3 w-3 text-[#6a6a6a]" />
            导出 Excel
          </button>
        </div>
      </div>

      {/* 统计卡片 — 4 列 */}
      <div className="grid grid-cols-4 gap-2.5">
        {reportCards.map((s) => (
          <div key={s.label} className="flex flex-col gap-4 bg-white p-4" style={{ border: `1px solid ${s.borderColor}` }}>
            <span className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{s.label}</span>
            <span
              className="text-[28px] font-bold tracking-tight"
              style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px', color: s.valueColor }}
            >
              {s.value}
            </span>
          </div>
        ))}
      </div>

      {/* 2 列图表占位区（保持不变） */}
      <div className="grid grid-cols-2 gap-4">
        <div className="flex flex-col gap-4 bg-white p-5" style={{ border: '1px solid #E8E8E8' }}>
          <span className="text-sm font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>
            DLP 规则严重级别分布
          </span>
          <div
            className="flex h-[200px] items-center justify-center"
            style={{ backgroundColor: '#F5F5F5', border: '1px dashed #D9D9D9' }}
            data-testid="chart-dlp-severity"
          >
            <span className="font-mono text-[10px] font-medium text-[#CCCCCC]">图表占位区域</span>
          </div>
        </div>
        <div className="flex flex-col gap-4 bg-white p-5" style={{ border: '1px solid #E8E8E8' }}>
          <span className="text-sm font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>
            AI 使用量趋势
          </span>
          <div
            className="flex h-[200px] items-center justify-center"
            style={{ backgroundColor: '#F5F5F5', border: '1px dashed #D9D9D9' }}
            data-testid="chart-ai-usage"
          >
            <span className="font-mono text-[10px] font-medium text-[#CCCCCC]">图表占位区域</span>
          </div>
        </div>
      </div>
    </div>
  )
}
