import { Scan } from 'lucide-react'
import { useState } from 'react'
import { TablePagination } from '@/components/ui/table-pagination'
import { ToggleSwitch } from '@/components/ui/toggle-switch'
import { StatusTag } from '@/components/ui/status-tag'

/* ── Mock 数据 ── */

const mockHistory = [
  { filename: '产品需求文档_v2.pdf', user: 'zhang.wei', department: '研发部', exportTime: '2024-07-10 14:32', result: 'success' },
  { filename: '财务报表_Q2.xlsx', user: 'li.ming', department: '财务部', exportTime: '2024-07-09 11:15', result: 'failed' },
]

const resultMap: Record<string, { label: string; color: string }> = {
  success: { label: '提取成功', color: '#0A6B3A' },
  failed: { label: '提取失败', color: '#CF1322' },
}

/* ── 组件 ── */

export default function WatermarkPage() {
  const [currentPage, setCurrentPage] = useState(1)

  return (
    <div className="flex flex-col gap-6">

      {/* 标题区 */}
      <div className="flex items-end justify-between">
        <div className="flex flex-col gap-2">
          <h1 className="text-[22px] font-bold tracking-tight text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px' }}>
            水印追踪
          </h1>
          <p className="font-mono text-xs text-[#999999]">为 AI 输出添加追踪水印，泄露时追溯来源</p>
        </div>
        <button className="flex items-center gap-2 bg-[#0A6B3A] px-4 py-2.5 font-mono text-[9px] font-semibold text-white">
          <Scan className="h-3 w-3" />
          提取水印
        </button>
      </div>

      {/* 2 列布局：配置 + 预览 */}
      <div className="grid grid-cols-2 gap-4">
        {/* 水印配置 */}
        <div className="flex flex-col gap-4 bg-white p-5" style={{ border: '1px solid #E8E8E8' }}>
          <span className="text-sm font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>
            水印配置
          </span>
          <div className="flex items-center justify-between">
            <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">启用水印</span>
            <ToggleSwitch on />
          </div>
          <div className="flex flex-col gap-1.5">
            <span className="font-mono text-[10px] font-medium text-[#999999]">水印类型</span>
            <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">文本水印</span>
          </div>
          <div className="flex flex-col gap-1.5">
            <span className="font-mono text-[10px] font-medium text-[#999999]">水印内容模板</span>
            <div className="border border-[#E8E8E8] bg-[#FAFAFA] px-3 py-2">
              <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">
                {'{{username}} - {{department}} - {{timestamp}}'}
              </span>
            </div>
          </div>
        </div>

        {/* 水印预览 */}
        <div className="flex flex-col gap-4 bg-white p-5" style={{ border: '1px solid #E8E8E8' }}>
          <span className="text-sm font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>
            水印预览
          </span>
          <div
            className="flex h-[160px] items-center justify-center"
            style={{ backgroundColor: '#F5F5F5', border: '1px dashed #D9D9D9' }}
            data-testid="watermark-preview"
          >
            <span className="font-mono text-[10px] font-medium text-[#CCCCCC]">水印预览区域</span>
          </div>
        </div>
      </div>

      {/* 水印提取历史表格 */}
      <div className="bg-white" style={{ border: '1px solid #E8E8E8' }}>
        <div className="px-4 py-3" style={{ borderBottom: '1px solid #E8E8E8' }}>
          <span className="text-sm font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>
            水印提取历史
          </span>
        </div>
        <div className="grid grid-cols-5 px-4 py-2.5" style={{ borderBottom: '1px solid #E8E8E8' }}>
          {['文件名', '追溯用户', '部门', '导出时间', '结果'].map((h) => (
            <span key={h} className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{h}</span>
          ))}
        </div>
        {mockHistory.map((r, i) => {
          const res = resultMap[r.result]
          return (
            <div
              key={r.filename}
              className="grid grid-cols-5 items-center px-4 py-3"
              style={{ borderBottom: i < mockHistory.length - 1 ? '1px solid #E8E8E8' : 'none' }}
            >
              <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{r.filename}</span>
              <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{r.user}</span>
              <span className="font-mono text-[10px] font-medium text-[#999999]">{r.department}</span>
              <span className="font-mono text-[10px] font-medium text-[#999999]">{r.exportTime}</span>
              <div><StatusTag label={res.label} color={res.color} /></div>
            </div>
          )
        })}
        <TablePagination current={currentPage} total={32} pageSize={10} onChange={setCurrentPage} />
      </div>
    </div>
  )
}
