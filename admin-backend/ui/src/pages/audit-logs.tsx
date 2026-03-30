import { Search, ChevronDown, Download } from 'lucide-react'

/* ── Mock 数据 ── */

const actionColorMap: Record<string, string> = {
  '创建用户': '#0A6B3A',
  '修改规则': '#D48700',
  '删除规则': '#CF1322',
  '登录系统': '#1677FF',
  '导出数据': '#D4B200',
}

const mockLogs = [
  { time: '2024-07-10 14:32:01', operator: 'admin', action: '创建用户', detail: '创建用户 zhang.wei，角色：开发工程师', ip: '192.168.1.100' },
  { time: '2024-07-10 13:15:42', operator: 'admin', action: '修改规则', detail: '修改 DLP 规则「身份证号码检测」严重级别为严重', ip: '192.168.1.100' },
  { time: '2024-07-10 11:08:33', operator: 'zhang.wei', action: '登录系统', detail: '用户 zhang.wei 通过密码登录', ip: '10.0.0.52' },
  { time: '2024-07-10 09:45:17', operator: 'admin', action: '删除规则', detail: '删除 DLP 规则「测试规则-临时」', ip: '192.168.1.100' },
  { time: '2024-07-09 17:30:00', operator: 'li.ming', action: '导出数据', detail: '导出对话审计报告 2024-07-09', ip: '10.0.0.88' },
]

/* ── 组件 ── */

export default function AuditLogsPage() {
  return (
    <div className="flex flex-col gap-6">

      {/* 标题区 */}
      <div className="flex items-end justify-between">
        <div className="flex flex-col gap-2">
          <h1 className="text-[22px] font-bold tracking-tight text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px' }}>
            审计日志
          </h1>
          <p className="font-mono text-xs text-[#999999]">查看和导出所有管理操作的审计记录</p>
        </div>
      </div>

      {/* 筛选栏 */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="flex w-[220px] items-center gap-2.5 border border-[#E8E8E8] bg-white px-3.5 py-2">
            <Search className="h-3 w-3 text-[#6a6a6a]" />
            <span className="font-mono text-[10px] font-medium text-[#CCCCCC]">搜索操作人或详情...</span>
          </div>
          <div className="flex items-center gap-2 border border-[#E8E8E8] bg-white px-3.5 py-2">
            <span className="font-mono text-[10px] font-medium text-[#999999]">操作类型</span>
            <ChevronDown className="h-3 w-3 text-[#999999]" />
          </div>
          <div className="flex items-center gap-2 border border-[#E8E8E8] bg-white px-3.5 py-2">
            <span className="font-mono text-[10px] font-medium text-[#999999]">日期范围</span>
            <ChevronDown className="h-3 w-3 text-[#999999]" />
          </div>
        </div>
        <button className="flex items-center gap-2 bg-[#0A6B3A] px-4 py-2.5 font-mono text-[9px] font-semibold text-white">
          <Download className="h-3 w-3" />
          导出 CSV
        </button>
      </div>

      {/* 表格 */}
      <div className="bg-white" style={{ border: '1px solid #E8E8E8' }}>
        {/* 表头 */}
        <div className="grid grid-cols-6 px-4 py-2.5" style={{ borderBottom: '1px solid #E8E8E8' }}>
          {['时间', '操作人', '操作类型', '详情', 'IP 地址', '操作'].map((h) => (
            <span key={h} className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{h}</span>
          ))}
        </div>

        {/* 数据行 */}
        {mockLogs.map((log, i) => (
          <div
            key={i}
            className="grid grid-cols-6 items-center px-4 py-3"
            style={{ borderBottom: i < mockLogs.length - 1 ? '1px solid #E8E8E8' : 'none' }}
          >
            <span className="font-mono text-[10px] font-medium text-[#999999]">{log.time}</span>
            <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{log.operator}</span>
            <span
              className="font-mono text-[10px] font-semibold"
              style={{ color: actionColorMap[log.action] ?? '#1A1A1A' }}
            >
              {log.action}
            </span>
            <span className="font-mono text-[10px] font-medium text-[#999999] truncate" title={log.detail}>{log.detail}</span>
            <span className="font-mono text-[10px] font-medium text-[#999999]">{log.ip}</span>
            <button className="font-mono text-[10px] font-semibold text-[#0A6B3A] text-left">详情</button>
          </div>
        ))}
      </div>

      {/* 分页 */}
      <div className="flex items-center justify-between">
        <span className="font-mono text-[10px] font-medium text-[#999999]">共 1,284 条记录</span>
        <div className="flex items-center gap-2">
          <button className="border border-[#E8E8E8] bg-white px-3 py-1.5 font-mono text-[9px] font-medium text-[#999999]">上一页</button>
          <span className="flex h-7 w-7 items-center justify-center bg-[#0A6B3A] font-mono text-[9px] font-semibold text-white">1</span>
          <button className="flex h-7 w-7 items-center justify-center border border-[#E8E8E8] bg-white font-mono text-[9px] font-medium text-[#999999]">2</button>
          <button className="flex h-7 w-7 items-center justify-center border border-[#E8E8E8] bg-white font-mono text-[9px] font-medium text-[#999999]">3</button>
          <button className="border border-[#E8E8E8] bg-white px-3 py-1.5 font-mono text-[9px] font-medium text-[#999999]">下一页</button>
        </div>
      </div>
    </div>
  )
}
