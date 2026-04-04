import { FileText, RefreshCw, Loader2 } from 'lucide-react'
import { useState, useEffect, useCallback } from 'react'
import { api } from '@/lib/api'

/* ── 类型 ── */

type Period = 'day' | 'week' | 'month'

interface AiUsagePoint {
  period: string
  conversation_count: number
  total_tokens: number
}

interface ModelCostItem {
  model_id: string
  display_name: string
  call_count: number
  input_tokens: number
  output_tokens: number
  cost_cents: number
}

interface DeptRankingItem {
  department_id: string
  department_name: string
  call_count: number
  total_tokens: number
  cost_cents: number
}

interface ReportStats {
  dlpRuleCount: number
  sensitiveOpsCount: number
  dictionaryCount: number
  totalKeywords: number
  clientTotal: number
}

const PERIOD_LABELS: Record<Period, string> = {
  day: '今日',
  week: '近 7 天',
  month: '近 30 天',
}

function formatCents(cents: number): string {
  return `¥${(cents / 100).toFixed(2)}`
}

/* ── 组件 ── */

export default function ReportsPage() {
  const [period, setPeriod] = useState<Period>('week')
  const [stats, setStats] = useState<ReportStats | null>(null)
  const [aiUsage, setAiUsage] = useState<AiUsagePoint[]>([])
  const [modelCost, setModelCost] = useState<ModelCostItem[]>([])
  const [deptRanking, setDeptRanking] = useState<DeptRankingItem[]>([])
  const [loading, setLoading] = useState(true)

  const loadReport = useCallback(async () => {
    setLoading(true)
    try {
      const [statsRes, aiRes, modelRes, deptRes] = await Promise.allSettled([
        Promise.allSettled([
          api.get('/dlp-rules'),
          api.get('/sensitive-operations'),
          api.get('/dlp-dictionaries'),
          api.get('/clients/stats'),
        ]),
        api.get(`/reports/ai-usage?period=${period}&limit=10`),
        api.get(`/reports/model-cost?period=${period}&limit=10`),
        api.get(`/reports/dept-ranking?period=${period}&limit=10`),
      ])

      // 基础统计
      if (statsRes.status === 'fulfilled') {
        const [rulesRes, opsRes, dictsRes, clientsRes] = statsRes.value
        const rules = rulesRes.status === 'fulfilled' ? (rulesRes.value.data.rules || []) : []
        const ops = opsRes.status === 'fulfilled' ? (opsRes.value.data.operations || []) : []
        const dicts = dictsRes.status === 'fulfilled' ? (dictsRes.value.data.dictionaries || []) : []
        const clientStats = clientsRes.status === 'fulfilled' ? clientsRes.value.data : { total: 0 }
        const totalKeywords = dicts.reduce((sum: number, d: { keyword_count?: number }) => sum + (d.keyword_count || 0), 0)
        setStats({ dlpRuleCount: rules.length, sensitiveOpsCount: ops.length, dictionaryCount: dicts.length, totalKeywords, clientTotal: clientStats.total || 0 })
      }

      if (aiRes.status === 'fulfilled') setAiUsage(aiRes.value.data.data || [])
      if (modelRes.status === 'fulfilled') setModelCost(modelRes.value.data.models || [])
      if (deptRes.status === 'fulfilled') setDeptRanking(deptRes.value.data.departments || [])
    } catch (err) {
      console.error('加载报表数据失败', err)
    } finally {
      setLoading(false)
    }
  }, [period])

  useEffect(() => { loadReport() }, [loadReport])

  const handleExportPdf = useCallback(() => {
    const periodLabel = PERIOD_LABELS[period]
    const generatedAt = new Date().toLocaleString('zh-CN')

      const tableStyle = `
        border-collapse: collapse; width: 100%; margin-bottom: 24px; font-size: 12px;
      `
      const thStyle = `
        border: 1px solid #ddd; padding: 8px 12px; background: #f5f5f5;
        text-align: left; font-weight: 600; color: #333;
      `
      const tdStyle = `border: 1px solid #ddd; padding: 8px 12px; color: #333;`

      const aiRows = aiUsage.slice(0, 20).map((r) => `
        <tr>
          <td style="${tdStyle}">${new Date(r.period).toLocaleDateString('zh-CN')}</td>
          <td style="${tdStyle}">${r.conversation_count}</td>
          <td style="${tdStyle}">${r.total_tokens.toLocaleString()}</td>
        </tr>
      `).join('')

      const modelRows = modelCost.slice(0, 20).map((r) => `
        <tr>
          <td style="${tdStyle}">${r.display_name}</td>
          <td style="${tdStyle}">${r.call_count}</td>
          <td style="${tdStyle}">${(r.input_tokens + r.output_tokens).toLocaleString()}</td>
          <td style="${tdStyle}">${formatCents(r.cost_cents)}</td>
        </tr>
      `).join('')

      const deptRows = deptRanking.slice(0, 20).map((r) => `
        <tr>
          <td style="${tdStyle}">${r.department_name}</td>
          <td style="${tdStyle}">${r.call_count}</td>
          <td style="${tdStyle}">${r.total_tokens.toLocaleString()}</td>
          <td style="${tdStyle}">${formatCents(r.cost_cents)}</td>
        </tr>
      `).join('')

      const html = `<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="UTF-8">
  <title>IronClaw 统计报表</title>
  <style>
    body { font-family: "PingFang SC", "Microsoft YaHei", sans-serif; padding: 32px; color: #1a1a1a; }
    h1 { font-size: 22px; margin-bottom: 4px; }
    .meta { font-size: 12px; color: #888; margin-bottom: 24px; }
    h2 { font-size: 15px; margin: 24px 0 8px; border-left: 3px solid #0A6B3A; padding-left: 8px; }
    @media print { body { padding: 16px; } }
  </style>
</head>
<body>
  <h1>IronClaw 统计报表</h1>
  <div class="meta">统计周期：${periodLabel} &nbsp;|&nbsp; 生成时间：${generatedAt}</div>

  <h2>AI 使用量趋势</h2>
  <table style="${tableStyle}">
    <thead><tr>
      <th style="${thStyle}">时间</th>
      <th style="${thStyle}">对话数</th>
      <th style="${thStyle}">Token 消耗</th>
    </tr></thead>
    <tbody>${aiRows || `<tr><td colspan="3" style="${tdStyle}; color:#999; text-align:center">暂无数据</td></tr>`}</tbody>
  </table>

  <h2>模型费用统计</h2>
  <table style="${tableStyle}">
    <thead><tr>
      <th style="${thStyle}">模型</th>
      <th style="${thStyle}">调用次数</th>
      <th style="${thStyle}">Token 消耗</th>
      <th style="${thStyle}">费用</th>
    </tr></thead>
    <tbody>${modelRows || `<tr><td colspan="4" style="${tdStyle}; color:#999; text-align:center">暂无数据</td></tr>`}</tbody>
  </table>

  <h2>部门使用量排行</h2>
  <table style="${tableStyle}">
    <thead><tr>
      <th style="${thStyle}">部门</th>
      <th style="${thStyle}">调用次数</th>
      <th style="${thStyle}">Token 消耗</th>
      <th style="${thStyle}">费用</th>
    </tr></thead>
    <tbody>${deptRows || `<tr><td colspan="4" style="${tdStyle}; color:#999; text-align:center">暂无数据</td></tr>`}</tbody>
  </table>
</body>
</html>`

      const win = window.open('', '_blank')
      if (!win) return
      win.document.write(html)
      win.document.close()
      win.focus()
      // document.write 后 onload 不可靠，用 setTimeout 等待渲染
      setTimeout(() => { win.print() }, 300)
  }, [period, aiUsage, modelCost, deptRanking])

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
        <div className="flex items-center gap-2">
          {/* 时间维度选择 */}
          <div className="flex border border-[#E8E8E8]">
            {(['day', 'week', 'month'] as Period[]).map((p) => (
              <button
                key={p}
                onClick={() => setPeriod(p)}
                className={`px-3 py-2 font-mono text-[9px] font-semibold transition-colors ${period === p ? 'bg-[#0A6B3A] text-white' : 'bg-white text-[#999]'}`}
              >
                {PERIOD_LABELS[p]}
              </button>
            ))}
          </div>
          <button onClick={loadReport} className="flex items-center gap-2 border border-[#E8E8E8] bg-white px-4 py-2.5 font-mono text-[9px] font-semibold text-[#1A1A1A]">
            {loading ? <Loader2 className="h-3 w-3 animate-spin text-[#6a6a6a]" /> : <RefreshCw className="h-3 w-3 text-[#6a6a6a]" />}
            刷新
          </button>
          <button
            onClick={handleExportPdf}
            disabled={loading}
            className="flex items-center gap-2 bg-[#0A6B3A] px-4 py-2.5 font-mono text-[9px] font-semibold text-white disabled:opacity-50"
          >
            <FileText className="h-3 w-3" />
            导出 PDF
          </button>
        </div>
      </div>

      {/* 基础统计卡片 */}
      <div className="grid grid-cols-4 gap-2.5">
        {reportCards.map((s) => (
          <div key={s.label} className="flex flex-col gap-4 bg-white p-4" style={{ border: `1px solid ${s.borderColor}` }}>
            <span className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{s.label}</span>
            <span className="text-[28px] font-bold tracking-tight" style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px', color: s.valueColor }}>
              {s.value}
            </span>
          </div>
        ))}
      </div>

      {/* AI 使用量趋势 */}
      <div className="bg-white p-5" style={{ border: '1px solid #E8E8E8' }}>
        <span className="text-sm font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>
          AI 使用量趋势
        </span>
        {loading ? (
          <div className="flex h-[120px] items-center justify-center"><Loader2 className="h-5 w-5 animate-spin text-[#999]" /></div>
        ) : aiUsage.length === 0 ? (
          <div className="flex h-[120px] items-center justify-center"><span className="font-mono text-[10px] text-[#999]">暂无数据</span></div>
        ) : (
          <div className="mt-3 overflow-x-auto">
            <table className="w-full">
              <thead>
                <tr className="border-b border-[#F0F0F0]">
                  {['时间', '对话数', 'Token 消耗'].map((h) => (
                    <th key={h} className="py-2 text-left font-mono text-[9px] font-semibold text-[#999]">{h}</th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {aiUsage.map((row, i) => (
                  <tr key={i} className="border-b border-[#F9F9F9]">
                    <td className="py-2 font-mono text-[10px] text-[#1A1A1A]">{new Date(row.period).toLocaleDateString('zh-CN')}</td>
                    <td className="py-2 font-mono text-[10px] text-[#1A1A1A]">{row.conversation_count}</td>
                    <td className="py-2 font-mono text-[10px] text-[#1A1A1A]">{row.total_tokens.toLocaleString()}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* 模型费用 + 部门排行 */}
      <div className="grid grid-cols-2 gap-4">
        <div className="bg-white p-5" style={{ border: '1px solid #E8E8E8' }}>
          <span className="text-sm font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>
            模型费用统计
          </span>
          {loading ? (
            <div className="flex h-[120px] items-center justify-center"><Loader2 className="h-5 w-5 animate-spin text-[#999]" /></div>
          ) : modelCost.length === 0 ? (
            <div className="flex h-[120px] items-center justify-center"><span className="font-mono text-[10px] text-[#999]">暂无数据</span></div>
          ) : (
            <div className="mt-3">
              <table className="w-full">
                <thead>
                  <tr className="border-b border-[#F0F0F0]">
                    {['模型', '调用次数', '费用'].map((h) => (
                      <th key={h} className="py-2 text-left font-mono text-[9px] font-semibold text-[#999]">{h}</th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {modelCost.map((row, i) => (
                    <tr key={i} className="border-b border-[#F9F9F9]">
                      <td className="py-2 font-mono text-[10px] text-[#1A1A1A] max-w-[120px] truncate">{row.display_name}</td>
                      <td className="py-2 font-mono text-[10px] text-[#1A1A1A]">{row.call_count}</td>
                      <td className="py-2 font-mono text-[10px] text-[#0A6B3A]">{formatCents(row.cost_cents)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>

        <div className="bg-white p-5" style={{ border: '1px solid #E8E8E8' }}>
          <span className="text-sm font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>
            部门使用量排行
          </span>
          {loading ? (
            <div className="flex h-[120px] items-center justify-center"><Loader2 className="h-5 w-5 animate-spin text-[#999]" /></div>
          ) : deptRanking.length === 0 ? (
            <div className="flex h-[120px] items-center justify-center"><span className="font-mono text-[10px] text-[#999]">暂无数据</span></div>
          ) : (
            <div className="mt-3">
              <table className="w-full">
                <thead>
                  <tr className="border-b border-[#F0F0F0]">
                    {['部门', 'Token 消耗', '费用'].map((h) => (
                      <th key={h} className="py-2 text-left font-mono text-[9px] font-semibold text-[#999]">{h}</th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {deptRanking.map((row, i) => (
                    <tr key={i} className="border-b border-[#F9F9F9]">
                      <td className="py-2 font-mono text-[10px] text-[#1A1A1A] max-w-[120px] truncate">{row.department_name}</td>
                      <td className="py-2 font-mono text-[10px] text-[#1A1A1A]">{row.total_tokens.toLocaleString()}</td>
                      <td className="py-2 font-mono text-[10px] text-[#0A6B3A]">{formatCents(row.cost_cents)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
