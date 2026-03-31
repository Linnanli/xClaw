import { Settings, Loader2, Save, RefreshCw, Search } from 'lucide-react'
import { useState, useEffect, useCallback } from 'react'
import { api } from '@/lib/api'
import { Dialog, DialogContent } from '@/components/ui/dialog'
import { FormLabel, StatusMessage } from '@/components/ui/form-helpers'

/* ── 类型 ── */

interface QuotaOverview {
  today_cost_cents: number
  today_tokens: number
  month_cost_cents: number
  month_tokens: number
  monthly_budget_cents: number | null
  budget_usage_pct: number | null
  active_models: number
}

interface RankingItem {
  name?: string
  model_id?: string
  cost_cents: number
  tokens: number
}

interface UsageRecord {
  id: string
  username: string
  model_id: string
  input_tokens: number
  output_tokens: number
  cost_cents: number
  created_at: string
  department_name: string | null
}

/* ── 格式化辅助 ── */

function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`
  return String(n)
}

function formatCents(cents: number): string {
  return `¥${(cents / 100).toLocaleString('zh-CN', { minimumFractionDigits: 0 })}`
}

/* ── 主组件 ── */

export default function QuotaPage() {
  const [overview, setOverview] = useState<QuotaOverview | null>(null)
  const [deptRanking, setDeptRanking] = useState<RankingItem[]>([])
  const [modelRanking, setModelRanking] = useState<RankingItem[]>([])
  const [loading, setLoading] = useState(true)
  const [configOpen, setConfigOpen] = useState(false)

  const loadData = useCallback(async () => {
    setLoading(true)
    try {
      const [ovRes, deptRes, modelRes] = await Promise.all([
        api.get('/quota/overview'),
        api.get('/quota/department-ranking'),
        api.get('/quota/model-ranking'),
      ])
      setOverview(ovRes.data)
      setDeptRanking(deptRes.data.ranking || [])
      setModelRanking(modelRes.data.ranking || [])
    } catch (err) {
      console.error('加载配额数据失败', err)
    } finally { setLoading(false) }
  }, [])

  useEffect(() => { loadData() }, [loadData])

  if (loading && !overview) {
    return <div className="flex justify-center py-32"><Loader2 className="h-5 w-5 animate-spin text-[#999]" /></div>
  }

  const ov = overview ?? { today_cost_cents: 0, today_tokens: 0, month_cost_cents: 0, month_tokens: 0, monthly_budget_cents: null, budget_usage_pct: null, active_models: 0 }

  const stats = [
    { label: '今日消耗', value: formatTokens(ov.today_tokens), sub: `${formatCents(ov.today_cost_cents)}`, color: '#1A1A1A' },
    { label: '本月消耗', value: formatTokens(ov.month_tokens), sub: `${formatCents(ov.month_cost_cents)}`, color: '#0A6B3A' },
    { label: '本月预算', value: ov.monthly_budget_cents != null ? formatCents(ov.monthly_budget_cents) : '未设置', sub: ov.budget_usage_pct != null ? `已用 ${ov.budget_usage_pct}%` : '', color: '#1A1A1A', subColor: (ov.budget_usage_pct ?? 0) > 80 ? '#CF1322' : '#D48700' },
    { label: '活跃模型', value: String(ov.active_models), sub: '个', color: '#1A1A1A' },
  ]

  const budgetPct = ov.budget_usage_pct ?? 0

  return (
    <div className="flex flex-col gap-6">
      <div className="flex items-end justify-between">
        <div className="flex flex-col gap-2">
          <h1 className="text-[22px] font-bold tracking-tight text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px' }}>配额管理</h1>
          <p className="font-mono text-xs text-[#999]">Token 使用量、费用统计与配额控制</p>
        </div>
        <button onClick={() => setConfigOpen(true)} className="flex items-center gap-2 border border-[#E8E8E8] bg-white px-4 py-2.5 font-mono text-[9px] font-semibold text-[#1A1A1A]">
          <Settings className="h-3 w-3 text-[#6a6a6a]" />
          配额设置
        </button>
      </div>

      {/* 统计卡片 */}
      <div className="grid grid-cols-4 gap-2.5">
        {stats.map(s => (
          <div key={s.label} className="flex flex-col gap-4 bg-white p-4" style={{ border: '1px solid #E8E8E8' }}>
            <span className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999]">{s.label}</span>
            <span className="text-[28px] font-bold tracking-tight" style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px', color: s.color }}>{s.value}</span>
            <span className="font-mono text-[10px] font-semibold" style={{ color: s.subColor ?? '#999' }}>{s.sub}</span>
          </div>
        ))}
      </div>

      {/* 预算进度条 */}
      {ov.monthly_budget_cents != null && (
        <div className="flex flex-col gap-3 bg-white p-5" style={{ border: '1px solid #E8E8E8' }}>
          <div className="flex items-center justify-between">
            <span className="text-sm font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>月度预算使用率</span>
            <span className="font-mono text-[10px] font-medium text-[#999]">{formatCents(ov.month_cost_cents)} / {formatCents(ov.monthly_budget_cents)}</span>
          </div>
          <div className="h-3 w-full overflow-hidden bg-[#F0F0F0]">
            <div className="h-full" style={{ width: `${Math.min(budgetPct, 100)}%`, background: budgetPct > 90 ? '#CF1322' : 'linear-gradient(90deg, #0A6B3A 0%, #D48700 100%)' }} />
          </div>
          <span className="font-mono text-[10px] font-semibold" style={{ color: budgetPct > 90 ? '#CF1322' : '#D48700' }}>{budgetPct}% 已使用</span>
        </div>
      )}

      {/* 排行区 */}
      <div className="grid grid-cols-2 gap-4">
        <RankingCard title="部门 Token 消耗" items={deptRanking} nameKey="name" />
        <RankingCard title="模型调用量" items={modelRanking} nameKey="model_id" />
      </div>

      {/* 费用明细记录（验收标准18#6） */}
      <UsageRecordsTable />

      <QuotaConfigDialog open={configOpen} onClose={() => setConfigOpen(false)} onSaved={loadData} />
    </div>
  )
}

/* ── 费用明细表格（验收标准18#6） ── */

function renderRecordsBody(loading: boolean, records: UsageRecord[]) {
  if (loading && records.length === 0) {
    return <div className="flex justify-center py-10"><Loader2 className="h-5 w-5 animate-spin text-[#999]" /></div>
  }
  if (records.length === 0) {
    return <div className="flex justify-center py-10"><span className="font-mono text-[10px] text-[#CCC]">暂无明细记录</span></div>
  }
  return records.map((r, i) => (
    <div key={r.id} className="grid grid-cols-7 items-center px-4 py-3" style={{ borderBottom: i < records.length - 1 ? '1px solid #E8E8E8' : 'none' }}>
      <span className="font-mono text-[10px] text-[#999]">
        {new Date(r.created_at).toLocaleString('zh-CN', { month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' })}
      </span>
      <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{r.username}</span>
      <span className="font-mono text-[10px] text-[#999]">{r.department_name ?? '-'}</span>
      <span className="font-mono text-[10px] text-[#1A1A1A]">{r.model_id}</span>
      <span className="font-mono text-[10px] text-[#999]">{formatTokens(r.input_tokens)}</span>
      <span className="font-mono text-[10px] text-[#999]">{formatTokens(r.output_tokens)}</span>
      <span className="font-mono text-[10px] font-semibold text-[#0A6B3A]">{formatCents(r.cost_cents)}</span>
    </div>
  ))
}

function UsageRecordsTable() {
  const [records, setRecords] = useState<UsageRecord[]>([])
  const [total, setTotal] = useState(0)
  const [page, setPage] = useState(1)
  const [loading, setLoading] = useState(false)
  const [search, setSearch] = useState('')
  const [modelFilter, setModelFilter] = useState('')
  const pageSize = 20

  const loadRecords = useCallback(async (p: number) => {
    setLoading(true)
    try {
      const params: Record<string, string> = { page: String(p), page_size: String(pageSize) }
      if (search.trim()) params.username = search.trim()
      if (modelFilter.trim()) params.model_id = modelFilter.trim()
      const res = await api.get('/quota/usage-records', { params })
      setRecords(res.data.records || [])
      setTotal(res.data.total || 0)
      setPage(p)
    } catch (err) {
      console.error('加载费用明细失败', err)
    } finally { setLoading(false) }
  }, [search, modelFilter])

  useEffect(() => { loadRecords(1) }, [loadRecords])

  const totalPages = Math.max(1, Math.ceil(total / pageSize))

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center justify-between">
        <span className="text-sm font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>费用明细</span>
        <div className="flex items-center gap-2">
          <div className="flex items-center gap-1.5 border border-[#E8E8E8] bg-white px-3 py-1.5">
            <Search className="h-3 w-3 text-[#999]" />
            <input
              value={search}
              onChange={e => setSearch(e.target.value)}
              placeholder="搜索用户"
              className="w-[100px] bg-transparent font-mono text-[10px] text-[#1A1A1A] outline-none placeholder:text-[#CCC]"
            />
          </div>
          <input
            value={modelFilter}
            onChange={e => setModelFilter(e.target.value)}
            placeholder="模型 ID"
            className="w-[120px] border border-[#E8E8E8] bg-white px-3 py-1.5 font-mono text-[10px] text-[#1A1A1A] outline-none placeholder:text-[#CCC]"
          />
          <button
            onClick={() => loadRecords(1)}
            className="flex items-center gap-1.5 border border-[#E8E8E8] bg-white px-3 py-1.5 font-mono text-[9px] font-semibold text-[#1A1A1A]"
          >
            {loading ? <Loader2 className="h-3 w-3 animate-spin" /> : <RefreshCw className="h-3 w-3 text-[#6a6a6a]" />}
            查询
          </button>
        </div>
      </div>

      <div className="bg-white" style={{ border: '1px solid #E8E8E8' }}>
        <div className="grid grid-cols-7 px-4 py-2.5" style={{ borderBottom: '1px solid #E8E8E8' }}>
          {['时间', '用户', '部门', '模型', '输入 Token', '输出 Token', '费用'].map(h => (
            <span key={h} className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999]">{h}</span>
          ))}
        </div>

        {renderRecordsBody(loading, records)}
      </div>

      {/* 分页 */}
      {totalPages > 1 && (
        <div className="flex items-center justify-between">
          <span className="font-mono text-[9px] text-[#999]">共 {total} 条</span>
          <div className="flex items-center gap-1">
            <button
              onClick={() => loadRecords(page - 1)}
              disabled={page <= 1}
              className="border border-[#E8E8E8] bg-white px-2.5 py-1 font-mono text-[9px] font-semibold text-[#1A1A1A] disabled:opacity-30"
            >上一页</button>
            <span className="px-2 font-mono text-[9px] text-[#999]">{page} / {totalPages}</span>
            <button
              onClick={() => loadRecords(page + 1)}
              disabled={page >= totalPages}
              className="border border-[#E8E8E8] bg-white px-2.5 py-1 font-mono text-[9px] font-semibold text-[#1A1A1A] disabled:opacity-30"
            >下一页</button>
          </div>
        </div>
      )}
    </div>
  )
}

/* ── 配额设置弹窗 ── */

function QuotaConfigDialog({ open, onClose, onSaved }: { open: boolean; onClose: () => void; onSaved: () => void }) {
  const [dailyLimit, setDailyLimit] = useState('')
  const [monthlyBudget, setMonthlyBudget] = useState('')
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState('')
  const [success, setSuccess] = useState('')

  useEffect(() => {
    if (!open) return
    setError(''); setSuccess('')
    api.get('/quota/config').then(res => {
      setDailyLimit(res.data.org_daily_limit_cents != null ? String(res.data.org_daily_limit_cents / 100) : '')
      setMonthlyBudget(res.data.monthly_budget_cents != null ? String(res.data.monthly_budget_cents / 100) : '')
    }).catch(() => setError('加载配置失败'))
  }, [open])

  const handleSave = async () => {
    setSaving(true); setError(''); setSuccess('')
    try {
      await api.put('/quota/config', {
        org_daily_limit_cents: dailyLimit ? Math.round(Number(dailyLimit) * 100) : null,
        monthly_budget_cents: monthlyBudget ? Math.round(Number(monthlyBudget) * 100) : null,
      })
      setSuccess('配额配置已保存')
      onSaved()
    } catch (err: any) {
      setError(err.response?.data?.error || '保存失败')
    } finally { setSaving(false) }
  }

  return (
    <Dialog open={open} onOpenChange={(v) => { if (!v) onClose() }}>
      <DialogContent className="sm:max-w-[480px] rounded-none p-0 gap-0 ring-0" style={{ border: '1px solid #E8E8E8' }} showCloseButton={false}>
        {error && <StatusMessage type="error" message={error} />}
        {success && <StatusMessage type="success" message={success} />}
        <div className="flex items-center justify-between px-6 py-5" style={{ borderBottom: '1px solid #E8E8E8' }}>
          <span className="font-mono text-xs font-semibold text-[#1A1A1A]">配额设置</span>
        </div>
        <div className="flex flex-col gap-4 px-6 py-5">
          <div className="flex flex-col gap-1.5">
            <FormLabel>组织每日限额（元）</FormLabel>
            <input type="number" value={dailyLimit} onChange={e => setDailyLimit(e.target.value)} min={0} step="0.01" placeholder="不填则不限制" className="border border-[#E8E8E8] bg-[#F5F5F5] px-3 py-2 font-mono text-[10px] text-[#1A1A1A] outline-none" />
            <span className="font-mono text-[9px] text-[#999]">所有用户每日总消耗上限</span>
          </div>
          <div className="flex flex-col gap-1.5">
            <FormLabel>月度预算（元）</FormLabel>
            <input type="number" value={monthlyBudget} onChange={e => setMonthlyBudget(e.target.value)} min={0} step="0.01" placeholder="不填则不限制" className="border border-[#E8E8E8] bg-[#F5F5F5] px-3 py-2 font-mono text-[10px] text-[#1A1A1A] outline-none" />
            <span className="font-mono text-[9px] text-[#999]">月度预算用于进度条展示和 90% 预警</span>
          </div>
        </div>
        <div className="flex justify-end gap-2 px-6 py-4" style={{ borderTop: '1px solid #E8E8E8' }}>
          <button onClick={onClose} className="border border-[#E8E8E8] bg-white px-4 py-2 font-mono text-[9px] font-semibold text-[#1A1A1A]">取消</button>
          <button onClick={handleSave} disabled={saving} className="flex items-center gap-1.5 bg-[#0A6B3A] px-4 py-2 font-mono text-[9px] font-semibold text-white disabled:opacity-50">
            {saving ? <Loader2 className="h-3 w-3 animate-spin" /> : <Save className="h-3 w-3" />}
            保存
          </button>
        </div>
      </DialogContent>
    </Dialog>
  )
}

function RankingCard({ title, items, nameKey }: { title: string; items: RankingItem[]; nameKey: 'name' | 'model_id' }) {
  return (
    <div className="flex flex-col gap-4 bg-white p-5" style={{ border: '1px solid #E8E8E8' }}>
      <span className="text-sm font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>{title}</span>
      <div className="flex flex-col gap-0">
        {items.length === 0 ? (
          <div className="py-4 text-center font-mono text-[10px] text-[#999]">暂无数据</div>
        ) : items.map((item, i) => (
          <div key={item[nameKey] ?? i} className="flex items-center justify-between px-2 py-3" style={{ borderBottom: i < items.length - 1 ? '1px solid #E8E8E8' : 'none' }}>
            <div className="flex items-center gap-3">
              <span className="font-mono text-[10px] font-bold text-[#0A6B3A]">{String(i + 1).padStart(2, '0')}</span>
              <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{item[nameKey] ?? '-'}</span>
            </div>
            <div className="flex items-center gap-3">
              <span className="font-mono text-[10px] font-medium text-[#999]">{formatTokens(item.tokens)}</span>
              <span className="font-mono text-[10px] font-semibold text-[#0A6B3A]">{formatCents(item.cost_cents)}</span>
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}
