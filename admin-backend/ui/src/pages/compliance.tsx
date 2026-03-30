import { FileText } from 'lucide-react'

/* ── Mock 数据 ── */

const classificationCards = [
  { level: '公开', rules: 4, blocks: 892, borderColor: '#0A6B3A' },
  { level: '内部', rules: 8, blocks: 2156, borderColor: '#D48700' },
  { level: '机密', rules: 7, blocks: 847, borderColor: '#D4A200' },
  { level: '绝密', rules: 5, blocks: 1284, borderColor: '#CF1322' },
]

const typeMap: Record<string, { label: string; color: string }> = {
  monthly: { label: '月度报告', color: '#0A6B3A' },
  quarterly: { label: '季度报告', color: '#1677FF' },
  annual: { label: '年度报告', color: '#D48700' },
}

const mockReports = [
  { name: '2024年6月合规报告', type: 'monthly', range: '2024-06-01 ~ 2024-06-30', generated: '2024-07-02 09:00' },
  { name: '2024年Q2合规报告', type: 'quarterly', range: '2024-04-01 ~ 2024-06-30', generated: '2024-07-05 14:30' },
  { name: '2023年度合规报告', type: 'annual', range: '2023-01-01 ~ 2023-12-31', generated: '2024-01-15 10:00' },
]

/* ── 辅助：渲染彩色标签 ── */

function StatusTag({ label, color }: { label: string; color: string }) {
  return (
    <span
      className="inline-block px-2 py-0.5 font-mono text-[9px] font-semibold"
      style={{ color, backgroundColor: `${color}1A`, border: `1px solid ${color}` }}
    >
      {label}
    </span>
  )
}

/* ── 组件 ── */

export default function CompliancePage() {
  return (
    <div className="flex flex-col gap-6">

      {/* 标题区 */}
      <div className="flex items-end justify-between">
        <div className="flex flex-col gap-2">
          <h1 className="text-[22px] font-bold tracking-tight text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px' }}>
            合规管理
          </h1>
          <p className="font-mono text-xs text-[#999999]">数据分类分级、合规报告与数据保留策略</p>
        </div>
        <button className="flex items-center gap-2 bg-[#0A6B3A] px-4 py-2.5 font-mono text-[9px] font-semibold text-white">
          <FileText className="h-3 w-3" />
          生成报告
        </button>
      </div>

      {/* 数据分级卡片 — 4 列 */}
      <div className="grid grid-cols-4 gap-2.5">
        {classificationCards.map((c) => (
          <div key={c.level} className="flex flex-col gap-3 bg-white p-4" style={{ border: `1px solid ${c.borderColor}` }}>
            <span className="text-sm font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>
              {c.level}
            </span>
            <div className="flex items-baseline gap-1">
              <span className="text-[28px] font-bold tracking-tight" style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px', color: c.borderColor }}>
                {c.rules}
              </span>
              <span className="font-mono text-[10px] font-medium text-[#999999]">规则</span>
            </div>
            <span className="font-mono text-[10px] font-medium text-[#999999]">{c.blocks.toLocaleString()} 次拦截</span>
          </div>
        ))}
      </div>

      {/* 历史合规报告表格 */}
      <div className="bg-white" style={{ border: '1px solid #E8E8E8' }}>
        <div className="px-4 py-3" style={{ borderBottom: '1px solid #E8E8E8' }}>
          <span className="text-sm font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>
            历史合规报告
          </span>
        </div>
        {/* 表头 */}
        <div className="grid grid-cols-5 px-4 py-2.5" style={{ borderBottom: '1px solid #E8E8E8' }}>
          {['报告名称', '类型', '时间范围', '生成时间', '操作'].map((h) => (
            <span key={h} className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{h}</span>
          ))}
        </div>
        {/* 数据行 */}
        {mockReports.map((r, i) => {
          const t = typeMap[r.type]
          return (
            <div
              key={r.name}
              className="grid grid-cols-5 items-center px-4 py-3"
              style={{ borderBottom: i < mockReports.length - 1 ? '1px solid #E8E8E8' : 'none' }}
            >
              <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{r.name}</span>
              <div><StatusTag label={t.label} color={t.color} /></div>
              <span className="font-mono text-[10px] font-medium text-[#999999]">{r.range}</span>
              <span className="font-mono text-[10px] font-medium text-[#999999]">{r.generated}</span>
              <button className="font-mono text-[10px] font-semibold text-[#0A6B3A] text-left">下载 PDF</button>
            </div>
          )
        })}
      </div>
    </div>
  )
}
