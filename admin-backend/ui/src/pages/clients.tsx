import { Search, Send, Monitor, LogOut, Trash2, ChevronDown } from 'lucide-react'
import { useState } from 'react'
import { TablePagination } from '@/components/ui/table-pagination'

/* ── Mock 数据 ── */

const clientStats = [
  { label: '总计', value: 89, dotColor: '', borderColor: '#E8E8E8' },
  { label: '在线', value: 47, dotColor: '#0A6B3A', borderColor: 'rgba(10,107,58,0.25)' },
  { label: '离线', value: 42, dotColor: '#999999', borderColor: '#E8E8E8' },
  { label: '需升级', value: 12, dotColor: '#D48700', borderColor: 'rgba(212,135,0,0.25)' },
]

const mockClients = [
  {
    name: 'DEV-WS-001',
    username: 'zhang.wei',
    status: 'online' as const,
    os: 'Windows 11',
    ip: '192.168.1.101',
    policyVersion: 'v2.4.1',
    fingerprint: true,
    needsUpgrade: false,
  },
  {
    name: 'DEV-WS-002',
    username: 'li.ming',
    status: 'online' as const,
    os: 'macOS 14.2',
    ip: '192.168.1.102',
    policyVersion: 'v2.4.1',
    fingerprint: true,
    needsUpgrade: true,
  },
  {
    name: 'MKT-WS-003',
    username: 'wang.fang',
    status: 'offline' as const,
    os: 'Windows 10',
    ip: '192.168.2.55',
    policyVersion: 'v2.3.0',
    fingerprint: false,
    needsUpgrade: true,
  },
]

/* ── 组件 ── */

export default function ClientsPage() {
  const [currentPage, setCurrentPage] = useState(1)

  return (
    <div className="flex flex-col gap-6">

      {/* 标题区 */}
      <div className="flex items-end justify-between">
        <div className="flex flex-col gap-2">
          <h1 className="text-[22px] font-bold tracking-tight text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px' }}>
            客户端管理
          </h1>
          <p className="font-mono text-xs text-[#999999]">管理已连接的桌面客户端、设备指纹与策略推送</p>
        </div>
        <div className="flex gap-2">
          <button className="flex items-center gap-2 bg-[#0A6B3A] px-4 py-2.5 font-mono text-[9px] font-semibold text-white">
            <Send className="h-3 w-3" />
            批量推送
          </button>
        </div>
      </div>

      {/* 统计卡片 — 4 列 */}
      <div className="grid grid-cols-4 gap-2.5">
        {clientStats.map((s) => (
          <div key={s.label} className="flex flex-col gap-4 bg-white p-4" style={{ border: `1px solid ${s.borderColor}` }}>
            <span className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{s.label}</span>
            <div className="flex items-center gap-2">
              {s.dotColor && <div className="h-2 w-2 rounded-full" style={{ backgroundColor: s.dotColor }} />}
              <span className="text-[28px] font-bold tracking-tight text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px' }}>
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
            <span className="font-mono text-[10px] font-medium text-[#CCCCCC]">搜索客户端名称...</span>
          </div>
          <div className="flex items-center gap-2 border border-[#E8E8E8] bg-white px-3.5 py-2">
            <span className="font-mono text-[10px] font-medium text-[#999999]">状态</span>
            <ChevronDown className="h-3 w-3 text-[#999999]" />
          </div>
          <div className="flex items-center gap-2 border border-[#E8E8E8] bg-white px-3.5 py-2">
            <span className="font-mono text-[10px] font-medium text-[#999999]">系统</span>
            <ChevronDown className="h-3 w-3 text-[#999999]" />
          </div>
        </div>
        <span className="font-mono text-[10px] font-medium text-[#999999]">共 89 个客户端</span>
      </div>

      {/* 表格 */}
      <div className="bg-white" style={{ border: '1px solid #E8E8E8' }}>
        {/* 表头 */}
        <div className="grid grid-cols-8 px-4 py-2.5" style={{ borderBottom: '1px solid #E8E8E8' }}>
          {['客户端', '用户', '状态', '系统', 'IP', '策略版本', '设备指纹', '操作'].map((h) => (
            <span key={h} className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{h}</span>
          ))}
        </div>

        {/* 数据行 */}
        {mockClients.map((c, i) => (
          <div
            key={c.name}
            className="grid grid-cols-8 items-center px-4 py-3"
            style={{ borderBottom: i < mockClients.length - 1 ? '1px solid #E8E8E8' : 'none' }}
          >
            {/* 客户端名称 */}
            <div className="flex items-center gap-2">
              <Monitor className="h-3.5 w-3.5 text-[#6a6a6a]" />
              <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{c.name}</span>
              {c.needsUpgrade && (
                <span
                  className="inline-block px-1.5 py-0 font-mono text-[8px] font-semibold"
                  style={{ color: '#D48700', backgroundColor: 'rgba(212,135,0,0.13)', border: '1px solid #D48700' }}
                >
                  需升级
                </span>
              )}
            </div>
            <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{c.username}</span>
            {/* 状态 */}
            <div className="flex items-center gap-1.5">
              <div
                className="h-2 w-2 rounded-full"
                style={{ backgroundColor: c.status === 'online' ? '#0A6B3A' : '#999999' }}
              />
              <span
                className="font-mono text-[10px] font-medium"
                style={{ color: c.status === 'online' ? '#1A1A1A' : '#999999' }}
              >
                {c.status === 'online' ? '在线' : '离线'}
              </span>
            </div>
            <span className="font-mono text-[10px] font-medium text-[#999999]">{c.os}</span>
            <span className="font-mono text-[10px] font-medium text-[#999999]">{c.ip}</span>
            {/* 策略版本 */}
            <div>
              <span
                className="inline-block px-2 py-0.5 font-mono text-[9px] font-semibold"
                style={{ color: '#1677FF', backgroundColor: 'rgba(22,119,255,0.13)', border: '1px solid #1677FF' }}
              >
                {c.policyVersion}
              </span>
            </div>
            {/* 设备指纹 */}
            <div>
              <span
                className="inline-block px-2 py-0.5 font-mono text-[9px] font-semibold"
                style={{
                  color: c.fingerprint ? '#0A6B3A' : '#999999',
                  backgroundColor: c.fingerprint ? 'rgba(10,107,58,0.13)' : 'rgba(153,153,153,0.13)',
                  border: `1px solid ${c.fingerprint ? '#0A6B3A' : '#999999'}`,
                }}
              >
                {c.fingerprint ? '已验证' : '未验证'}
              </span>
            </div>
            {/* 操作 */}
            <div className="flex items-center gap-3">
              <button className="text-[#6a6a6a] hover:text-[#0A6B3A]" title="推送策略">
                <Send className="h-3.5 w-3.5" />
              </button>
              <button className="text-[#6a6a6a] hover:text-[#D48700]" title="下线">
                <LogOut className="h-3.5 w-3.5" />
              </button>
              <button className="text-[#6a6a6a] hover:text-[#CF1322]" title="删除">
                <Trash2 className="h-3.5 w-3.5" />
              </button>
            </div>
          </div>
        ))}
        <TablePagination current={currentPage} total={156} pageSize={10} onChange={setCurrentPage} />
      </div>
    </div>
  )
}
