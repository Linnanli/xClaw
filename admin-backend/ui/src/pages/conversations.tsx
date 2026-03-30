import { Search, ChevronDown, Download } from 'lucide-react'

/* ── Mock 数据 ── */

const convStats = [
  { label: '今日对话', value: '342', dotColor: '', valueColor: '#1A1A1A', borderColor: '#E8E8E8' },
  { label: 'Token 消耗', value: '1.2M', dotColor: '', valueColor: '#0A6B3A', borderColor: '#E8E8E8' },
  { label: 'DLP 标记', value: '18', dotColor: '#CF1322', valueColor: '#CF1322', borderColor: 'rgba(207,19,34,0.25)' },
  { label: '活跃用户', value: '67', dotColor: '', valueColor: '#1A1A1A', borderColor: '#E8E8E8' },
]

const mockConversations = [
  { username: 'zhang.wei', topic: '产品需求分析', message_count: 24, token_usage: '18.5K', model: 'deepseek-chat', dlp_flagged: true, time: '14:32' },
  { username: 'li.ming', topic: '代码审查辅助', message_count: 56, token_usage: '42.1K', model: 'qwen-plus', dlp_flagged: false, time: '14:15' },
  { username: 'wang.fang', topic: 'HR 政策咨询', message_count: 12, token_usage: '8.2K', model: 'moonshot-v1', dlp_flagged: true, time: '13:48' },
  { username: 'chen.jing', topic: '市场报告生成', message_count: 8, token_usage: '35.7K', model: 'deepseek-chat', dlp_flagged: false, time: '13:20' },
]

/* ── 组件 ── */

export default function ConversationsPage() {
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
        <div className="flex gap-2">
          <button className="flex items-center gap-2 border border-[#E8E8E8] bg-white px-4 py-2.5 font-mono text-[9px] font-semibold text-[#1A1A1A]">
            <Download className="h-3 w-3 text-[#6a6a6a]" />
            导出报告
          </button>
        </div>
      </div>

      {/* 统计卡片 — 4 列 */}
      <div className="grid grid-cols-4 gap-2.5">
        {convStats.map((s) => (
          <div key={s.label} className="flex flex-col gap-4 bg-white p-4" style={{ border: `1px solid ${s.borderColor}` }}>
            <span className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{s.label}</span>
            <div className="flex items-center gap-2">
              {s.dotColor && <div className="h-2 w-2 rounded-full" style={{ backgroundColor: s.dotColor }} />}
              <span
                className="text-[28px] font-bold tracking-tight"
                style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px', color: s.valueColor }}
              >
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
            <span className="font-mono text-[10px] font-medium text-[#CCCCCC]">搜索用户或主题...</span>
          </div>
          <div className="flex items-center gap-2 border border-[#E8E8E8] bg-white px-3.5 py-2">
            <span className="font-mono text-[10px] font-medium text-[#999999]">DLP</span>
            <ChevronDown className="h-3 w-3 text-[#999999]" />
          </div>
        </div>
        <span className="font-mono text-[10px] font-medium text-[#999999]">共 1,284 条对话</span>
      </div>

      {/* 表格 */}
      <div className="bg-white" style={{ border: '1px solid #E8E8E8' }}>
        {/* 表头 */}
        <div className="grid grid-cols-7 px-4 py-2.5" style={{ borderBottom: '1px solid #E8E8E8' }}>
          {['用户', '对话主题', '消息数', 'TOKEN', '模型', 'DLP', '时间'].map((h) => (
            <span key={h} className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{h}</span>
          ))}
        </div>

        {/* 数据行 */}
        {mockConversations.map((c, i) => (
          <div
            key={i}
            className="grid grid-cols-7 items-center px-4 py-3"
            style={{ borderBottom: i < mockConversations.length - 1 ? '1px solid #E8E8E8' : 'none' }}
          >
            <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{c.username}</span>
            <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{c.topic}</span>
            <span className="font-mono text-[10px] font-medium text-[#999999]">{c.message_count}</span>
            <span className="font-mono text-[10px] font-medium text-[#999999]">{c.token_usage}</span>
            <div>
              <span
                className="inline-block px-2 py-0.5 font-mono text-[9px] font-semibold"
                style={{ color: '#1677FF', backgroundColor: 'rgba(22,119,255,0.1)', border: '1px solid #1677FF' }}
              >
                {c.model}
              </span>
            </div>
            <div>
              {c.dlp_flagged ? (
                <span
                  className="inline-block px-2 py-0.5 font-mono text-[9px] font-semibold"
                  style={{ color: '#CF1322', backgroundColor: 'rgba(207,19,34,0.1)', border: '1px solid #CF1322' }}
                >
                  标记
                </span>
              ) : (
                <span className="font-mono text-[10px] font-medium text-[#999999]">—</span>
              )}
            </div>
            <span className="font-mono text-[10px] font-medium text-[#999999]">{c.time}</span>
          </div>
        ))}
      </div>
    </div>
  )
}
