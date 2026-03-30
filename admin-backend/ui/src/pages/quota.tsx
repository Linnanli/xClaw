import { Settings } from 'lucide-react'

/* ── Mock 数据 ── */

const quotaStats = [
  { label: '今日消耗', value: '2.4M', sub: 'Token', valueColor: '#1A1A1A', borderColor: '#E8E8E8' },
  { label: '本月消耗', value: '48.7M', sub: 'Token', valueColor: '#0A6B3A', borderColor: '#E8E8E8' },
  { label: '本月预算', value: '¥2,400', sub: '已用 68%', subColor: '#D48700', valueColor: '#1A1A1A', borderColor: '#E8E8E8' },
  { label: '活跃模型', value: '5', sub: '个提供商', valueColor: '#1A1A1A', borderColor: '#E8E8E8' },
]

const deptRanking = [
  { rank: '01', name: '研发部', tokens: '18.2M' },
  { rank: '02', name: '产品部', tokens: '12.5M' },
  { rank: '03', name: '市场部', tokens: '9.8M' },
  { rank: '04', name: '运营部', tokens: '5.1M' },
]

const modelRanking = [
  { name: 'deepseek-chat', tokens: '28.4M', cost: '¥842' },
  { name: 'qwen-plus', tokens: '12.1M', cost: '¥485' },
  { name: 'moonshot-v1', tokens: '5.8M', cost: '¥232' },
  { name: 'ollama-local', tokens: '2.4M', cost: '¥0' },
]

/* ── 组件 ── */

export default function QuotaPage() {
  return (
    <div className="flex flex-col gap-6">

      {/* 标题区 */}
      <div className="flex items-end justify-between">
        <div className="flex flex-col gap-2">
          <h1 className="text-[22px] font-bold tracking-tight text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px' }}>
            配额管理
          </h1>
          <p className="font-mono text-xs text-[#999999]">Token 使用量、费用统计与配额控制</p>
        </div>
        <div className="flex gap-2">
          <button className="flex items-center gap-2 border border-[#E8E8E8] bg-white px-4 py-2.5 font-mono text-[9px] font-semibold text-[#1A1A1A]">
            <Settings className="h-3 w-3 text-[#6a6a6a]" />
            配额设置
          </button>
        </div>
      </div>

      {/* 统计卡片 — 4 列 */}
      <div className="grid grid-cols-4 gap-2.5">
        {quotaStats.map((s) => (
          <div key={s.label} className="flex flex-col gap-4 bg-white p-4" style={{ border: `1px solid ${s.borderColor}` }}>
            <span className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{s.label}</span>
            <span
              className="text-[28px] font-bold tracking-tight"
              style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px', color: s.valueColor }}
            >
              {s.value}
            </span>
            <span
              className="font-mono text-[10px] font-semibold"
              style={{ color: s.subColor ?? '#999999' }}
            >
              {s.sub}
            </span>
          </div>
        ))}
      </div>

      {/* 预算进度条 */}
      <div className="flex flex-col gap-3 bg-white p-5" style={{ border: '1px solid #E8E8E8' }}>
        <div className="flex items-center justify-between">
          <span className="text-sm font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>
            月度预算使用率
          </span>
          <span className="font-mono text-[10px] font-medium text-[#999999]">¥1,632 / ¥2,400</span>
        </div>
        <div className="h-3 w-full overflow-hidden bg-[#F0F0F0]" data-testid="budget-progress">
          <div
            className="h-full"
            style={{
              width: '68%',
              background: 'linear-gradient(90deg, #0A6B3A 0%, #D48700 100%)',
            }}
          />
        </div>
        <span className="font-mono text-[10px] font-semibold text-[#D48700]">68% 已使用</span>
      </div>

      {/* 排行区 — 2 列 */}
      <div className="grid grid-cols-2 gap-4">
        {/* 部门 Token 消耗 */}
        <div className="flex flex-col gap-4 bg-white p-5" style={{ border: '1px solid #E8E8E8' }}>
          <span className="text-sm font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>
            部门 Token 消耗
          </span>
          <div className="flex flex-col gap-0">
            {deptRanking.map((d, i) => (
              <div
                key={d.rank}
                className="flex items-center justify-between px-2 py-3"
                style={{ borderBottom: i < deptRanking.length - 1 ? '1px solid #E8E8E8' : 'none' }}
              >
                <div className="flex items-center gap-3">
                  <span className="font-mono text-[10px] font-bold text-[#0A6B3A]">{d.rank}</span>
                  <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{d.name}</span>
                </div>
                <span className="font-mono text-[10px] font-semibold text-[#1A1A1A]">{d.tokens}</span>
              </div>
            ))}
          </div>
        </div>

        {/* 模型调用量 */}
        <div className="flex flex-col gap-4 bg-white p-5" style={{ border: '1px solid #E8E8E8' }}>
          <span className="text-sm font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>
            模型调用量
          </span>
          <div className="flex flex-col gap-0">
            {modelRanking.map((m, i) => (
              <div
                key={m.name}
                className="flex items-center justify-between px-2 py-3"
                style={{ borderBottom: i < modelRanking.length - 1 ? '1px solid #E8E8E8' : 'none' }}
              >
                <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{m.name}</span>
                <div className="flex items-center gap-3">
                  <span className="font-mono text-[10px] font-medium text-[#999999]">{m.tokens}</span>
                  <span className="font-mono text-[10px] font-semibold text-[#0A6B3A]">{m.cost}</span>
                </div>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  )
}
