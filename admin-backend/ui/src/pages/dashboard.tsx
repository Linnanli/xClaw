import { TrendingUp, RefreshCw, Download, Bell } from 'lucide-react'

/* ── Mock 数据（开发阶段，后续替换为 API 调用）── */

const stats = [
  { label: '总用户数', value: '1,284', change: '+8.2%', changeColor: '#0A6B3A', borderColor: '#E8E8E8' },
  { label: '在线客户端', value: '47', change: '今日 +3', changeColor: '#0A6B3A', borderColor: '#E8E8E8' },
  { label: 'DLP 拦截', value: '128', change: '今日 +24', changeColor: '#CF1322', borderColor: '#E8E8E8' },
  { label: '今日 AI 对话', value: '342', change: '1.2M Token', changeColor: '#999999', borderColor: 'rgba(10,107,58,0.25)', valueColor: '#0A6B3A' },
  { label: '敏感操作', value: '36', change: '今日 +5', changeColor: '#D48700', borderColor: 'rgba(212,135,0,0.25)' },
]

const alertStat = { value: 7, sub: '3 严重' }

const activityBars = [
  { label: '周一', h: 50 }, { label: '周二', h: 75 }, { label: '周三', h: 40 },
  { label: '周四', h: 90 }, { label: '周五', h: 60 }, { label: '周六', h: 30 }, { label: '周日', h: 20 },
]

const dlpBars = [
  { label: '周一', h: 25 }, { label: '周二', h: 45 }, { label: '周三', h: 35 },
  { label: '周四', h: 75 }, { label: '周五', h: 55 }, { label: '周六', h: 15 }, { label: '周日', h: 10 },
]

const logs = [
  { time: '14:32:01', user: 'zhang.wei', action: 'DLP_拦截', status: '已拦截', color: '#CF1322', bg: 'rgba(207,19,34,0.13)' },
  { time: '14:28:45', user: 'li.ming', action: '用户登录', status: '成功', color: '#0A6B3A', bg: 'rgba(10,107,58,0.13)' },
  { time: '14:21:17', user: '系统', action: '策略推送', status: '已推送', color: '#1677FF', bg: 'rgba(22,119,255,0.13)' },
  { time: '14:15:03', user: 'wang.fang', action: 'AI 对话', status: 'DLP 标记', color: '#D48700', bg: 'rgba(212,135,0,0.13)' },
]

/* ── 组件 ── */

export default function DashboardPage() {
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
          <button className="flex items-center gap-2 border border-[#E8E8E8] bg-white px-4 py-2.5 font-mono text-[9px] font-semibold text-[#1A1A1A]">
            <RefreshCw className="h-3 w-3 text-[#6a6a6a]" />
            刷新
          </button>
          <button className="flex items-center gap-2 bg-[#0A6B3A] px-4 py-2.5 font-mono text-[9px] font-semibold text-white">
            <Download className="h-3 w-3" />
            导出
          </button>
        </div>
      </div>

      {/* 统计卡片 — 6 列 */}
      <div className="grid grid-cols-6 gap-2.5">
        {stats.map((s) => (
          <div key={s.label} className="flex flex-col gap-4 bg-white p-4" style={{ border: `1px solid ${s.borderColor}` }}>
            <span className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{s.label}</span>
            <span className="text-[28px] font-bold tracking-tight" style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px', color: s.valueColor ?? '#1A1A1A' }}>
              {s.value}
            </span>
            <div className="flex items-center gap-1.5">
              {s.changeColor !== '#999999' && <TrendingUp className="h-3 w-3" style={{ color: s.changeColor }} />}
              <span className="font-mono text-[10px] font-semibold" style={{ color: s.changeColor }}>{s.change}</span>
            </div>
          </div>
        ))}

        {/* 未处理告警 — 特殊样式 */}
        <div className="flex flex-col gap-4 bg-white p-4" style={{ border: '1px solid rgba(207,19,34,0.25)' }}>
          <span className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">未处理告警</span>
          <div className="flex items-center gap-2">
            <div className="h-2 w-2 rounded-full bg-[#CF1322]" />
            <span className="text-[28px] font-bold tracking-tight text-[#CF1322]" style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px' }}>
              {alertStat.value}
            </span>
          </div>
          <span className="font-mono text-[10px] font-semibold text-[#CF1322]">{alertStat.sub}</span>
        </div>
      </div>

      {/* 图表区 — 2 列 */}
      <div className="grid grid-cols-2 gap-4">
        {/* 用户活动趋势 */}
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

        {/* DLP 拦截趋势 */}
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
          {/* 表头 */}
          <div className="grid grid-cols-4 px-4 py-2.5" style={{ borderBottom: '1px solid #E8E8E8' }}>
            {['时间', '操作人', '操作类型', '状态'].map((h) => (
              <span key={h} className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{h}</span>
            ))}
          </div>

          {/* 数据行 */}
          {logs.map((log, i) => (
            <div
              key={i}
              className="grid grid-cols-4 items-center px-4 py-3"
              style={{ borderBottom: i < logs.length - 1 ? '1px solid #E8E8E8' : 'none' }}
            >
              <span className="font-mono text-[10px] font-medium text-[#999999]">{log.time}</span>
              <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{log.user}</span>
              <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{log.action}</span>
              <div>
                <span
                  className="inline-block rounded-none px-2 py-0.5 font-mono text-[9px] font-semibold"
                  style={{ color: log.color, backgroundColor: log.bg, border: `1px solid ${log.color}` }}
                >
                  {log.status}
                </span>
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  )
}
