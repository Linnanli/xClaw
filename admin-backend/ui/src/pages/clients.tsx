import { Search, Send, Monitor, LogOut, Trash2, Loader2 } from 'lucide-react'
import { useState, useEffect, useCallback, useMemo } from 'react'
import { api } from '@/lib/api'
import { TablePagination } from '@/components/ui/table-pagination'

/* ── 类型 ── */

interface Client {
  id: string
  client_name?: string
  username?: string
  online: boolean
  os?: string
  ip_address?: string
  version?: string
  policy_version?: string
  fingerprint?: boolean
  last_activity: string
}

interface ClientStats {
  total: number
  online: number
  offline: number
  by_version: Array<{ version: string; count: number }>
}

/* ── 组件 ── */

export default function ClientsPage() {
  const [clients, setClients] = useState<Client[]>([])
  const [stats, setStats] = useState<ClientStats | null>(null)
  const [loading, setLoading] = useState(true)
  const [currentPage, setCurrentPage] = useState(1)
  const [searchText, setSearchText] = useState('')
  const pageSize = 10

  const loadClients = useCallback(async () => {
    setLoading(true)
    try {
      const [clientsRes, statsRes] = await Promise.allSettled([
        api.get('/clients'),
        api.get('/clients/stats'),
      ])
      if (clientsRes.status === 'fulfilled') setClients(clientsRes.value.data.clients || [])
      if (statsRes.status === 'fulfilled') setStats(statsRes.value.data)
    } catch (err) {
      console.error('加载客户端数据失败', err)
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => { loadClients() }, [loadClients])

  const handleDelete = useCallback(async (id: string) => {
    try {
      await api.delete(`/clients/${id}`)
      loadClients()
    } catch (err) {
      console.error('删除客户端失败', err)
    }
  }, [loadClients])

  const filteredClients = useMemo(() => {
    if (!searchText) return clients
    const kw = searchText.toLowerCase()
    return clients.filter((c) =>
      (c.client_name || '').toLowerCase().includes(kw) ||
      (c.username || '').toLowerCase().includes(kw) ||
      (c.ip_address || '').toLowerCase().includes(kw)
    )
  }, [clients, searchText])

  const paged = useMemo(() => {
    const start = (currentPage - 1) * pageSize
    return filteredClients.slice(start, start + pageSize)
  }, [filteredClients, currentPage])

  const needsUpgradeCount = useMemo(() => {
    if (!stats?.by_version || stats.by_version.length <= 1) return 0
    const sorted = [...stats.by_version].sort((a, b) => b.version.localeCompare(a.version))
    const latest = sorted[0]?.version
    return clients.filter((c) => c.version && c.version !== latest).length
  }, [clients, stats])

  const statCards = [
    { label: '总计', value: stats?.total ?? 0, dotColor: '', borderColor: '#E8E8E8' },
    { label: '在线', value: stats?.online ?? 0, dotColor: '#0A6B3A', borderColor: 'rgba(10,107,58,0.25)' },
    { label: '离线', value: stats?.offline ?? 0, dotColor: '#999999', borderColor: '#E8E8E8' },
    { label: '需升级', value: needsUpgradeCount, dotColor: '#D48700', borderColor: 'rgba(212,135,0,0.25)' },
  ]

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
        {statCards.map((s) => (
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
            <input
              type="text"
              value={searchText}
              onChange={(e) => { setSearchText(e.target.value); setCurrentPage(1) }}
              placeholder="搜索客户端名称..."
              className="w-full bg-transparent font-mono text-[10px] font-medium text-[#1A1A1A] placeholder:text-[#CCCCCC] outline-none"
            />
          </div>
        </div>
        <span className="font-mono text-[10px] font-medium text-[#999999]">共 {filteredClients.length} 个客户端</span>
      </div>

      {/* 表格 */}
      <div className="bg-white" style={{ border: '1px solid #E8E8E8' }}>
        <div className="grid grid-cols-8 px-4 py-2.5" style={{ borderBottom: '1px solid #E8E8E8' }}>
          {['客户端', '用户', '状态', '系统', 'IP', '策略版本', '设备指纹', '操作'].map((h) => (
            <span key={h} className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{h}</span>
          ))}
        </div>

        {loading ? (
          <div className="flex items-center justify-center py-10">
            <Loader2 className="h-5 w-5 animate-spin text-[#999999]" />
          </div>
        ) : paged.length === 0 ? (
          <div className="flex items-center justify-center py-10">
            <span className="font-mono text-[10px] text-[#CCCCCC]">暂无客户端数据</span>
          </div>
        ) : (
          paged.map((c, i) => {
            const isOnline = c.online
            const hasFingerprint = c.fingerprint ?? false
            return (
              <div
                key={c.id}
                className="grid grid-cols-8 items-center px-4 py-3"
                style={{ borderBottom: i < paged.length - 1 ? '1px solid #E8E8E8' : 'none' }}
              >
                <div className="flex items-center gap-2">
                  <Monitor className="h-3.5 w-3.5 text-[#6a6a6a]" />
                  <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{c.client_name || c.id.slice(0, 8)}</span>
                </div>
                <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{c.username || '—'}</span>
                <div className="flex items-center gap-1.5">
                  <div className="h-2 w-2 rounded-full" style={{ backgroundColor: isOnline ? '#0A6B3A' : '#999999' }} />
                  <span className="font-mono text-[10px] font-medium" style={{ color: isOnline ? '#1A1A1A' : '#999999' }}>
                    {isOnline ? '在线' : '离线'}
                  </span>
                </div>
                <span className="font-mono text-[10px] font-medium text-[#999999]">{c.os || '—'}</span>
                <span className="font-mono text-[10px] font-medium text-[#999999]">{c.ip_address || '—'}</span>
                <div>
                  {c.policy_version ? (
                    <span className="inline-block px-2 py-0.5 font-mono text-[9px] font-semibold" style={{ color: '#1677FF', backgroundColor: 'rgba(22,119,255,0.13)', border: '1px solid #1677FF' }}>
                      {c.policy_version}
                    </span>
                  ) : (
                    <span className="font-mono text-[10px] text-[#CCCCCC]">—</span>
                  )}
                </div>
                <div>
                  <span
                    className="inline-block px-2 py-0.5 font-mono text-[9px] font-semibold"
                    style={{
                      color: hasFingerprint ? '#0A6B3A' : '#999999',
                      backgroundColor: hasFingerprint ? 'rgba(10,107,58,0.13)' : 'rgba(153,153,153,0.13)',
                      border: `1px solid ${hasFingerprint ? '#0A6B3A' : '#999999'}`,
                    }}
                  >
                    {hasFingerprint ? '已验证' : '未验证'}
                  </span>
                </div>
                <div className="flex items-center gap-3">
                  <button className="text-[#6a6a6a] hover:text-[#0A6B3A]" title="推送策略">
                    <Send className="h-3.5 w-3.5" />
                  </button>
                  <button className="text-[#6a6a6a] hover:text-[#D48700]" title="下线">
                    <LogOut className="h-3.5 w-3.5" />
                  </button>
                  <button className="text-[#6a6a6a] hover:text-[#CF1322]" title="删除" onClick={() => handleDelete(c.id)}>
                    <Trash2 className="h-3.5 w-3.5" />
                  </button>
                </div>
              </div>
            )
          })
        )}
        <TablePagination current={currentPage} total={filteredClients.length} pageSize={pageSize} onChange={setCurrentPage} />
      </div>
    </div>
  )
}
