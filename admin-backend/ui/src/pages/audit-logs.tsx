import { Search, Download, Loader2 } from 'lucide-react'
import { useState, useEffect, useCallback, useMemo } from 'react'
import { api } from '@/lib/api'
import { TablePagination } from '@/components/ui/table-pagination'

/* ── 类型 ── */

interface AuditLog {
  id: string
  username: string | null
  user_id: string | null
  action: string
  details: string
  ip_address?: string
  created_at: string
}

/* ── 操作类型颜色 ── */

const actionColorMap: Record<string, string> = {
  create: '#0A6B3A',
  update: '#D48700',
  delete: '#CF1322',
  login: '#1677FF',
  export: '#D4B200',
  toggle: '#D48700',
  enable: '#0A6B3A',
  disable: '#D48700',
  block: '#CF1322',
  scan: '#1677FF',
}

function getActionColor(action: string): string {
  const lower = action.toLowerCase()
  for (const [key, color] of Object.entries(actionColorMap)) {
    if (lower.includes(key)) return color
  }
  return '#1A1A1A'
}

/* ── 组件 ── */

export default function AuditLogsPage() {
  const [logs, setLogs] = useState<AuditLog[]>([])
  const [total, setTotal] = useState(0)
  const [currentPage, setCurrentPage] = useState(1)
  const [loading, setLoading] = useState(true)
  const [searchText, setSearchText] = useState('')
  const pageSize = 10

  const loadLogs = useCallback(async (page: number) => {
    setLoading(true)
    try {
      const res = await api.get('/audit-logs', { params: { page, page_size: pageSize } })
      setLogs(res.data.logs || [])
      setTotal(res.data.total || 0)
    } catch (err) {
      console.error('加载审计日志失败', err)
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => { loadLogs(currentPage) }, [currentPage, loadLogs])

  const handlePageChange = useCallback((page: number) => { setCurrentPage(page) }, [])

  const handleExport = useCallback(async () => {
    try {
      const res = await api.get('/audit-logs/export', { responseType: 'blob' })
      const blob = new Blob([res.data], { type: 'text/csv;charset=utf-8' })
      const url = URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = `audit-logs-${new Date().toISOString().slice(0, 10)}.csv`
      a.click()
      URL.revokeObjectURL(url)
    } catch (err) {
      console.error('导出审计日志失败', err)
    }
  }, [])

  /* 前端过滤当前页数据 */
  const filteredLogs = useMemo(() => {
    if (!searchText) return logs
    const kw = searchText.toLowerCase()
    return logs.filter((log) =>
      (log.username || '').toLowerCase().includes(kw) ||
      log.action.toLowerCase().includes(kw) ||
      log.details.toLowerCase().includes(kw)
    )
  }, [logs, searchText])

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
            <input
              type="text"
              value={searchText}
              onChange={(e) => setSearchText(e.target.value)}
              placeholder="搜索操作人或详情..."
              className="w-full bg-transparent font-mono text-[10px] font-medium text-[#1A1A1A] placeholder:text-[#CCCCCC] outline-none"
            />
          </div>
        </div>
        <button
          className="flex items-center gap-2 bg-[#0A6B3A] px-4 py-2.5 font-mono text-[9px] font-semibold text-white"
          onClick={handleExport}
        >
          <Download className="h-3 w-3" />
          导出 CSV
        </button>
      </div>

      {/* 表格 */}
      <div className="bg-white" style={{ border: '1px solid #E8E8E8' }}>
        <div className="grid grid-cols-6 px-4 py-2.5" style={{ borderBottom: '1px solid #E8E8E8' }}>
          {['时间', '操作人', '操作类型', '详情', 'IP 地址', '操作'].map((h) => (
            <span key={h} className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{h}</span>
          ))}
        </div>

        {loading ? (
          <div className="flex items-center justify-center py-10">
            <Loader2 className="h-5 w-5 animate-spin text-[#999999]" />
          </div>
        ) : filteredLogs.length === 0 ? (
          <div className="flex items-center justify-center py-10">
            <span className="font-mono text-[10px] text-[#CCCCCC]">暂无审计日志</span>
          </div>
        ) : (
          filteredLogs.map((log, i) => (
            <div
              key={log.id}
              className="grid grid-cols-6 items-center px-4 py-3"
              style={{ borderBottom: i < filteredLogs.length - 1 ? '1px solid #E8E8E8' : 'none' }}
            >
              <span className="font-mono text-[10px] font-medium text-[#999999]">
                {new Date(log.created_at).toLocaleString('zh-CN', { year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', second: '2-digit' })}
              </span>
              <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{log.username || '系统'}</span>
              <span className="font-mono text-[10px] font-semibold" style={{ color: getActionColor(log.action) }}>
                {log.action}
              </span>
              <span className="font-mono text-[10px] font-medium text-[#999999] truncate" title={log.details}>{log.details}</span>
              <span className="font-mono text-[10px] font-medium text-[#999999]">{log.ip_address || '—'}</span>
              <button className="font-mono text-[10px] font-semibold text-[#0A6B3A] text-left">详情</button>
            </div>
          ))
        )}
        <TablePagination current={currentPage} total={total} pageSize={pageSize} onChange={handlePageChange} />
      </div>
    </div>
  )
}
