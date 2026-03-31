import { TrendingUp, RefreshCw, Download, Loader2, Coins } from 'lucide-react'
import { useState, useEffect, useCallback } from 'react'
import { api } from '@/lib/api'

/* ── 类型 ── */

interface DashboardStats {
  total_users: number
  online_clients: number
  dlp_blocked_today: number
  sensitive_ops_today: number
}

interface ActivityLog {
  id: string
  username: string | null
  action: string
  details: string
  created_at: string
}

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

/* ── 图表 mock（API 无此数据） ── */

const activityBars = [
  { label: '周一', h: 50 }, { label: '周二', h: 75 }, { label: '周三', h: 40 },
  { label: '周四', h: 90 }, { label: '周五', h: 60 }, { label: '周六', h: 30 }, { label: '周日', h: 20 },
]

const dlpBars = [
  { label: '周一', h: 25 }, { label: '周二', h: 45 }, { label: '周三', h: 35 },
  { label: '周四', h: 75 }, { label: '周五', h: 55 }, { label: '周六', h: 15 }, { label: '周日', h: 10 },
]

/* ── 状态标签颜色映射 ── */

function getStatusStyle(action: string): { color: string; bg: string } {
  const lower = action.toLowerCase()
  if (lower.includes('拦截') || lower.includes('block') || lower.includes('delete')) return { color: '#CF1322', bg: 'rgba(207,19,34,0.13)' }
  if (lower.includes('登录') || lower.includes('login') || lower.includes('create')) return { color: '#0A6B3A', bg: 'rgba(10,107,58,0.13)' }
  if (lower.includes('推送') || lower.includes('push') || lower.includes('update')) return { color: '#1677FF', bg: 'rgba(22,119,255,0.13)' }
  if (lower.includes('标记') || lower.includes('warn')) return { color: '#D48700', bg: 'rgba(212,135,0,0.13)' }
  return { color: '#999999', bg: 'rgba(153,153,153,0.13)' }
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

/* ── 组件 ── */

export default function DashboardPage() {
  const [tab, setTab] = useState<'overview' | 'quota'>('overview')
  const [stats, setStats] = useState<DashboardStats | null>(null)
  const [logs, setLogs] = useState<ActivityLog[]>([])
  const [loading, setLoading] = useState(true)

  // 配额数据
  const [quotaOverview, setQuotaOverview] = useState<QuotaOverview | null>(null)
  const [deptRanking, setDeptRanking] = useState<RankingItem[]>([])
  const [modelRanking, setModelRanking] = useState<RankingItem[]>([])
  const [quotaLoading, setQuotaLoading] = useState(false)

  const loadDashboard = useCallback(async () => {
    setLoading(true)
    try {
      const [statsRes, activityRes] = await Promise.allSettled([
        api.get('/dashboard/stats'),
        api.get('/dashboard/activity'),
      ])
      if (statsRes.status === 'fulfilled') setStats(statsRes.value.data)
      if (activityRes.status === 'fulfilled') setLogs(activityRes.value.data.logs || [])
    } catch (err) {
      console.error('加载仪表盘数据失败', err)
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => { loadDashboard() }, [loadDashboard])

  const loadQuota = useCallback(async () => {
    setQuotaLoading(true)
    try {
      const [ovRes, deptRes, modelRes] = await Promise.all([
        api.get('/quota/overview'),
        api.get('/quota/department-ranking'),
        api.get('/quota/model-ranking'),
      ])
      setQuotaOverview(ovRes.data)
      setDeptRanking(deptRes.data.ranking || [])
      setModelRanking(modelRes.data.ranking || [])
    } catch (err) {
      console.error('加载配额数据失败', err)
    } finally { setQuotaLoading(false) }
  }, [])

  useEffect(() => { if (tab === 'quota' && !quotaOverview) loadQuota() }, [tab, quotaOverview, loadQuota])

  const statCards = [
    { label: '总用户数', value: stats ? stats.total_users.toLocaleString() : '—', change: '', changeColor: '#999999', borderColor: '#E8E8E8' },
    { label: '在线客户端', value: stats ? String(stats.online_clients) : '—', change: '', changeColor: '#0A6B3A', borderColor: '#E8E8E8' },
    { label: 'DLP 拦截', value: stats ? String(stats.dlp_blocked_today) : '—', change: '今日', changeColor: '#CF1322', borderColor: '#E8E8E8' },
    { label: '敏感操作', value: stats ? String(stats.sensitive_ops_today) : '—', change: '今日', changeColor: '#D48700', borderColor: 'rgba(212,135,0,0.25)' },
    { label: '今日 AI 对话', value: '342', change: '1.2M Token', changeColor: '#999999', borderColor: 'rgba(10,107,58,0.25)', valueColor: '#0A6B3A' },
    { label: '系统健康度', value: '99.8%', change: '正常', changeColor: '#0A6B3A', borderColor: '#E8E8E8' },
  ]

  return (
    <div className="flex flex-col gap-6">

      {/* 标题区 */}
      <div className="flex items-end justify-between">
        <div className="flex flex-col gap-2">
          <h1 className="text-[22px] font-bold tracking-tight text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px' }}>
            总览
          </h1>
          <p className="font-mono text-xs text-[#999999]">监控 DLP、客户端、AI 使用与安全指标</p>
        </div>
        <div className="flex gap-2">
          <button
            className="flex items-center gap-2 border border-[#E8E8E8] bg-white px-4 py-2.5 font-mono text-[9px] font-semibold text-[#1A1A1A]"
            onClick={tab === 'overview' ? loadDashboard : loadQuota}
          >
            {(loading || quotaLoading) ? <Loader2 className="h-3 w-3 animate-spin text-[#6a6a6a]" /> : <RefreshCw className="h-3 w-3 text-[#6a6a6a]" />}
            刷新
          </button>
          <button className="flex items-center gap-2 bg-[#0A6B3A] px-4 py-2.5 font-mono text-[9px] font-semibold text-white">
            <Download className="h-3 w-3" />
            导出
          </button>
        </div>
      </div>

      {/* Tab 栏 */}
      <div className="flex gap-0" style={{ borderBottom: '1px solid #E8E8E8' }}>
        {([['overview', '系统概览'], ['quota', '费用统计']] as const).map(([key, label]) => (
          <button
            key={key}
            onClick={() => setTab(key)}
            className="px-5 py-2.5 font-mono text-[10px] font-semibold transition-colors"
            style={{
              color: tab === key ? '#0A6B3A' : '#999',
              borderBottom: tab === key ? '2px solid #0A6B3A' : '2px solid transparent',
              marginBottom: '-1px',
            }}
          >
            {label}
          </button>
        ))}
      </div>

      {tab === 'overview' ? (
        <OverviewTab stats={stats} logs={logs} loading={loading} statCards={statCards} />
      ) : (
        <QuotaTab overview={quotaOverview} deptRanking={deptRanking} modelRanking={modelRanking} loading={quotaLoading} />
      )}
    </div>
  )
}

/* ── 系统概览 Tab ── */

function OverviewTab({ stats, logs, loading, statCards }: {
  stats: DashboardStats | null; logs: ActivityLog[]; loading: boolean
  statCards: { label: string; value: string; change: string; changeColor: string; borderColor: string; valueColor?: string }[]
}) {
  return (
    <>
      {/* 统计卡片 — 6 列 */}
      <div className="grid grid-cols-6 gap-2.5">
        {statCards.map((s) => (
          <div key={s.label} className="flex flex-col gap-4 bg-white p-4" style={{ border: `1px solid ${s.borderColor}` }}>
            <span className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{s.label}</span>
            <span className="text-[28px] font-bold tracking-tight" style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px', color: s.valueColor ?? '#1A1A1A' }}>
              {loading && !stats ? '—' : s.value}
            </span>
            {s.change && (
              <div className="flex items-center gap-1.5">
                {s.changeColor !== '#999999' && <TrendingUp className="h-3 w-3" style={{ color: s.changeColor }} />}
                <span className="font-mono text-[10px] font-semibold" style={{ color: s.changeColor }}>{s.change}</span>
              </div>
            )}
          </div>
        ))}
      </div>

      {/* 图表区 — 2 列（保持占位） */}
      <div className="grid grid-cols-2 gap-4">
        <div className="flex flex-col gap-5 bg-white p-5" style={{ border: '1px solid #E8E8E8' }}>
          <div className="flex items-center justify-between">
            <div className="flex flex-col gap-1.5">
              <span className="font-mono text-[9px] font-medium text-[#0A6B3A]">// 图表_01</span>
              <span className="text-base font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>用户活动趋势</span>
            </div>
            <span className="font-mono text-[9px] font-medium text-[#999999]">近 7 天</span>
          </div>
          <div className="flex h-[100px] items-end gap-2.5" style={{ borderBottom: '1px solid #E8E8E8' }}>
            {activityBars.map((bar) => (
              <div key={bar.label} className="flex flex-1 flex-col items-center justify-end gap-1.5" style={{ height: '100%' }}>
                <div className="w-full bg-[#0A6B3A]" style={{ height: `${bar.h}%`, opacity: bar.h < 35 ? 0.4 : 1 }} />
                <span className="font-mono text-[8px] font-medium text-[#999999]">{bar.label}</span>
              </div>
            ))}
          </div>
        </div>

        <div className="flex flex-col gap-5 bg-white p-5" style={{ border: '1px solid #E8E8E8' }}>
          <div className="flex items-center justify-between">
            <div className="flex flex-col gap-1.5">
              <span className="font-mono text-[9px] font-medium text-[#0A6B3A]">// 图表_02</span>
              <span className="text-base font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>DLP 拦截趋势</span>
            </div>
            <span className="font-mono text-[9px] font-medium text-[#999999]">近 7 天</span>
          </div>
          <div className="flex h-[100px] items-end gap-2.5" style={{ borderBottom: '1px solid #E8E8E8' }}>
            {dlpBars.map((bar) => (
              <div key={bar.label} className="flex flex-1 flex-col items-center justify-end gap-1.5" style={{ height: '100%' }}>
                <div className="w-full bg-[#CF1322]" style={{ height: `${bar.h}%`, opacity: bar.h < 20 ? 0.4 : 1 }} />
                <span className="font-mono text-[8px] font-medium text-[#999999]">{bar.label}</span>
              </div>
            ))}
          </div>
        </div>
      </div>

      {/* 最近操作记录 */}
      <div className="flex flex-col gap-4">
        <div className="flex items-center justify-between">
          <div className="flex flex-col gap-1.5">
            <span className="font-mono text-[9px] font-medium text-[#0A6B3A]">// 日志_01</span>
            <span className="text-base font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>最近操作记录</span>
          </div>
          <span className="font-mono text-[10px] font-semibold text-[#0A6B3A] cursor-pointer">查看全部 &gt;&gt;</span>
        </div>

        <div className="bg-white" style={{ border: '1px solid #E8E8E8' }}>
          <div className="grid grid-cols-4 px-4 py-2.5" style={{ borderBottom: '1px solid #E8E8E8' }}>
            {['时间', '操作人', '操作类型', '状态'].map((h) => (
              <span key={h} className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{h}</span>
            ))}
          </div>

          {loading && logs.length === 0 ? (
            <div className="flex items-center justify-center py-10">
              <Loader2 className="h-5 w-5 animate-spin text-[#999999]" />
            </div>
          ) : logs.length === 0 ? (
            <div className="flex items-center justify-center py-10">
              <span className="font-mono text-[10px] text-[#CCCCCC]">暂无操作记录</span>
            </div>
          ) : (
            logs.map((log, i) => {
              const style = getStatusStyle(log.action)
              return (
                <div
                  key={log.id}
                  className="grid grid-cols-4 items-center px-4 py-3"
                  style={{ borderBottom: i < logs.length - 1 ? '1px solid #E8E8E8' : 'none' }}
                >
                  <span className="font-mono text-[10px] font-medium text-[#999999]">
                    {new Date(log.created_at).toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit', second: '2-digit' })}
                  </span>
                  <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{log.username || '系统'}</span>
                  <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{log.action}</span>
                  <div>
                    <span
                      className="inline-block rounded-none px-2 py-0.5 font-mono text-[9px] font-semibold"
                      style={{ color: style.color, backgroundColor: style.bg, border: `1px solid ${style.color}` }}
                    >
                      {log.details?.slice(0, 20) || log.action}
                    </span>
                  </div>
                </div>
              )
            })
          )}
        </div>
      </div>
    </>
  )
}

/* ── 费用统计 Tab ── */

function QuotaTab({ overview, deptRanking, modelRanking, loading }: {
  overview: QuotaOverview | null; deptRanking: RankingItem[]; modelRanking: RankingItem[]; loading: boolean
}) {
  if (loading && !overview) {
    return <div className="flex justify-center py-16"><Loader2 className="h-5 w-5 animate-spin text-[#999]" /></div>
  }

  const ov = overview ?? { today_cost_cents: 0, today_tokens: 0, month_cost_cents: 0, month_tokens: 0, monthly_budget_cents: null, budget_usage_pct: null, active_models: 0 }

  const stats = [
    { label: '今日消耗', value: formatTokens(ov.today_tokens), sub: formatCents(ov.today_cost_cents), color: '#1A1A1A' },
    { label: '本月消耗', value: formatTokens(ov.month_tokens), sub: formatCents(ov.month_cost_cents), color: '#0A6B3A' },
    { label: '本月预算', value: ov.monthly_budget_cents != null ? formatCents(ov.monthly_budget_cents) : '未设置', sub: ov.budget_usage_pct != null ? `已用 ${ov.budget_usage_pct}%` : '', color: '#1A1A1A', subColor: (ov.budget_usage_pct ?? 0) > 80 ? '#CF1322' : '#D48700' },
    { label: '活跃模型', value: String(ov.active_models), sub: '个', color: '#1A1A1A' },
  ]

  const budgetPct = ov.budget_usage_pct ?? 0

  return (
    <>
      <div className="grid grid-cols-4 gap-2.5">
        {stats.map(s => (
          <div key={s.label} className="flex flex-col gap-4 bg-white p-4" style={{ border: '1px solid #E8E8E8' }}>
            <span className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999]">{s.label}</span>
            <span className="text-[28px] font-bold tracking-tight" style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px', color: s.color }}>{s.value}</span>
            <span className="font-mono text-[10px] font-semibold" style={{ color: s.subColor ?? '#999' }}>{s.sub}</span>
          </div>
        ))}
      </div>

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

      <div className="grid grid-cols-2 gap-4">
        <RankingCard title="部门费用消耗" items={deptRanking} nameKey="name" />
        <RankingCard title="模型调用量" items={modelRanking} nameKey="model_id" />
      </div>
    </>
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
