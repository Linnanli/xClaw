import { Settings } from 'lucide-react'
import { useState } from 'react'
import { TablePagination } from '@/components/ui/table-pagination'
import { StatusTag } from '@/components/ui/status-tag'

/* ── Mock 数据 ── */

const approvalStats = [
  { label: '待审批', value: '5', dotColor: '#CF1322', valueColor: '#CF1322', borderColor: 'rgba(207,19,34,0.25)' },
  { label: '已批准', value: '42', dotColor: '', valueColor: '#0A6B3A', borderColor: '#E8E8E8' },
  { label: '已拒绝', value: '8', dotColor: '', valueColor: '#999999', borderColor: '#E8E8E8' },
  { label: '已过期', value: '3', dotColor: '', valueColor: '#D48700', borderColor: 'rgba(212,135,0,0.25)' },
]

const statusMap: Record<string, { label: string; color: string }> = {
  pending: { label: '待审批', color: '#CF1322' },
  approved: { label: '已批准', color: '#0A6B3A' },
  rejected: { label: '已拒绝', color: '#999999' },
  expired: { label: '已过期', color: '#D48700' },
}

const mockTickets = [
  { applicant: 'zhang.wei', operation: '导出客户数据 (CSV)', apply_time: '01-15 14:32', expire_time: '01-16 14:32', expire_warn: true, status: 'pending' },
  { applicant: 'li.ming', operation: '批量删除 DLP 规则 (5条)', apply_time: '01-15 11:20', expire_time: '01-16 11:20', expire_warn: true, status: 'pending' },
  { applicant: 'wang.fang', operation: '修改系统安全配置', apply_time: '01-14 16:45', expire_time: '01-15 16:45', expire_warn: false, status: 'approved' },
  { applicant: 'chen.jing', operation: '访问绝密级知识库', apply_time: '01-13 09:30', expire_time: '01-14 09:30', expire_warn: false, status: 'rejected' },
]

/* ── 组件 ── */

export default function ApprovalsPage() {
  const [currentPage, setCurrentPage] = useState(1)

  return (
    <div className="flex flex-col gap-6">

      {/* 标题区 */}
      <div className="flex items-end justify-between">
        <div className="flex flex-col gap-2">
          <h1 className="text-[22px] font-bold tracking-tight text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px' }}>
            审批工单
          </h1>
          <p className="font-mono text-xs text-[#999999]">高风险操作审批流程管理</p>
        </div>
        <div className="flex gap-2">
          <button className="flex items-center gap-2 border border-[#E8E8E8] bg-white px-4 py-2.5 font-mono text-[9px] font-semibold text-[#1A1A1A]">
            <Settings className="h-3 w-3 text-[#6a6a6a]" />
            审批规则配置
          </button>
        </div>
      </div>

      {/* 统计卡片 — 4 列 */}
      <div className="grid grid-cols-4 gap-2.5">
        {approvalStats.map((s) => (
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

      {/* 表格 */}
      <div className="bg-white" style={{ border: '1px solid #E8E8E8' }}>
        {/* 表头 */}
        <div className="grid grid-cols-6 px-4 py-2.5" style={{ borderBottom: '1px solid #E8E8E8' }}>
          {['申请人', '操作类型', '申请时间', '过期时间', '状态', '操作'].map((h) => (
            <span key={h} className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{h}</span>
          ))}
        </div>

        {/* 数据行 */}
        {mockTickets.map((t, i) => {
          const st = statusMap[t.status]
          return (
            <div
              key={i}
              className="grid grid-cols-6 items-center px-4 py-3"
              style={{ borderBottom: i < mockTickets.length - 1 ? '1px solid #E8E8E8' : 'none' }}
            >
              <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{t.applicant}</span>
              <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{t.operation}</span>
              <span className="font-mono text-[10px] font-medium text-[#999999]">{t.apply_time}</span>
              <span
                className="font-mono text-[10px] font-medium"
                style={{ color: t.expire_warn ? '#D48700' : '#999999' }}
              >
                {t.expire_time}
              </span>
              <div><StatusTag label={st.label} color={st.color} /></div>
              <div className="flex items-center gap-2">
                {t.status === 'pending' ? (
                  <>
                    <button className="px-3 py-1 font-mono text-[9px] font-semibold text-white bg-[#0A6B3A]">
                      批准
                    </button>
                    <button
                      className="px-3 py-1 font-mono text-[9px] font-semibold bg-white"
                      style={{ color: '#CF1322', border: '1px solid #CF1322' }}
                    >
                      拒绝
                    </button>
                  </>
                ) : (
                  <button className="font-mono text-[10px] font-semibold text-[#0A6B3A]">
                    查看
                  </button>
                )}
              </div>
            </div>
          )
        })}
        <TablePagination current={currentPage} total={23} pageSize={10} onChange={setCurrentPage} />
      </div>
    </div>
  )
}
