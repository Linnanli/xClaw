import { Search, ChevronDown, Bell, Plus } from 'lucide-react'

/* ── Mock 数据 ── */

const alertStats = [
  { label: '未处理', value: '7', dotColor: '#CF1322', valueColor: '#CF1322', borderColor: 'rgba(207,19,34,0.25)' },
  { label: '处理中', value: '3', dotColor: '', valueColor: '#D48700', borderColor: 'rgba(212,135,0,0.25)' },
  { label: '今日告警', value: '23', dotColor: '', valueColor: '#1A1A1A', borderColor: '#E8E8E8' },
  { label: '已关闭', value: '156', dotColor: '', valueColor: '#1A1A1A', borderColor: '#E8E8E8' },
]

const severityMap: Record<string, { label: string; color: string }> = {
  critical: { label: '严重', color: '#CF1322' },
  high: { label: '高危', color: '#D48700' },
  medium: { label: '中危', color: '#D4B200' },
  low: { label: '低危', color: '#0A6B3A' },
}

const statusMap: Record<string, { label: string; color: string }> = {
  pending: { label: '未处理', color: '#CF1322' },
  in_progress: { label: '处理中', color: '#D48700' },
  closed: { label: '已关闭', color: '#0A6B3A' },
}

const mockAlerts = [
  { time: '14:32:01', severity: 'critical', rule_name: 'DLP 拦截阈值超限', trigger_detail: 'zhang.wei 触发身份证规则 12 次/小时', status: 'pending' },
  { time: '13:15:42', severity: 'high', rule_name: '异常登录检测', trigger_detail: 'li.ming 从异地 IP 登录 (上海→广州)', status: 'in_progress' },
  { time: '11:08:33', severity: 'medium', rule_name: 'Token 配额预警', trigger_detail: '研发部 Token 消耗达到月配额 80%', status: 'closed' },
  { time: '09:45:17', severity: 'critical', rule_name: '模型服务异常', trigger_detail: 'DeepSeek API 连续失败 5 次', status: 'pending' },
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

export default function AlertsPage() {
  return (
    <div className="flex flex-col gap-6">

      {/* 标题区 */}
      <div className="flex items-end justify-between">
        <div className="flex flex-col gap-2">
          <h1 className="text-[22px] font-bold tracking-tight text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px' }}>
            告警中心
          </h1>
          <p className="font-mono text-xs text-[#999999]">安全事件检测、告警规则管理与多渠道通知</p>
        </div>
        <div className="flex gap-2">
          <button className="flex items-center gap-2 border border-[#E8E8E8] bg-white px-4 py-2.5 font-mono text-[9px] font-semibold text-[#1A1A1A]">
            <Bell className="h-3 w-3 text-[#6a6a6a]" />
            告警规则
          </button>
          <button className="flex items-center gap-2 bg-[#0A6B3A] px-4 py-2.5 font-mono text-[9px] font-semibold text-white">
            <Plus className="h-3 w-3" />
            新建规则
          </button>
        </div>
      </div>

      {/* 统计卡片 — 4 列 */}
      <div className="grid grid-cols-4 gap-2.5">
        {alertStats.map((s) => (
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
            <span className="font-mono text-[10px] font-medium text-[#CCCCCC]">搜索告警...</span>
          </div>
          <div className="flex items-center gap-2 border border-[#E8E8E8] bg-white px-3.5 py-2">
            <span className="font-mono text-[10px] font-medium text-[#999999]">级别</span>
            <ChevronDown className="h-3 w-3 text-[#999999]" />
          </div>
          <div className="flex items-center gap-2 border border-[#E8E8E8] bg-white px-3.5 py-2">
            <span className="font-mono text-[10px] font-medium text-[#999999]">状态</span>
            <ChevronDown className="h-3 w-3 text-[#999999]" />
          </div>
        </div>
        <span className="font-mono text-[10px] font-medium text-[#999999]">共 189 条告警</span>
      </div>

      {/* 表格 */}
      <div className="bg-white" style={{ border: '1px solid #E8E8E8' }}>
        {/* 表头 */}
        <div className="grid grid-cols-6 px-4 py-2.5" style={{ borderBottom: '1px solid #E8E8E8' }}>
          {['时间', '级别', '规则名称', '触发详情', '状态', '操作'].map((h) => (
            <span key={h} className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{h}</span>
          ))}
        </div>

        {/* 数据行 */}
        {mockAlerts.map((a, i) => {
          const sev = severityMap[a.severity]
          const st = statusMap[a.status]
          return (
            <div
              key={i}
              className="grid grid-cols-6 items-center px-4 py-3"
              style={{ borderBottom: i < mockAlerts.length - 1 ? '1px solid #E8E8E8' : 'none' }}
            >
              <span className="font-mono text-[10px] font-medium text-[#999999]">{a.time}</span>
              <div><StatusTag label={sev.label} color={sev.color} /></div>
              <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{a.rule_name}</span>
              <span className="font-mono text-[10px] font-medium text-[#999999]">{a.trigger_detail}</span>
              <div><StatusTag label={st.label} color={st.color} /></div>
              <div>
                <button className="font-mono text-[10px] font-semibold text-[#0A6B3A]">
                  {a.status === 'pending' ? '处理' : '查看'}
                </button>
              </div>
            </div>
          )
        })}
      </div>
    </div>
  )
}
