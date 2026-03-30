import { Plus, Upload, Search } from 'lucide-react'

/* ── Mock 数据 ── */

const mockKBs = [
  { name: '产品文档库', description: '产品需求、设计文档与技术规范', document_count: 42, chunk_count: 1284, access: '研发部可访问', enabled: true },
  { name: 'HR 政策库', description: '人事政策、员工手册与培训资料', document_count: 18, chunk_count: 567, access: '全员可访问', enabled: true },
  { name: '法务合规库', description: '法律法规、合规要求与审计标准', document_count: 8, chunk_count: 0, access: '法务部可访问', enabled: false },
]

const fileTypeMap: Record<string, { label: string; color: string }> = {
  PDF: { label: 'PDF', color: '#CF1322' },
  DOCX: { label: 'DOCX', color: '#1677FF' },
  MD: { label: 'MD', color: '#0A6B3A' },
  TXT: { label: 'TXT', color: '#999999' },
}

const docStatusMap: Record<string, { label: string; color: string }> = {
  completed: { label: '已完成', color: '#0A6B3A' },
  processing: { label: '处理中', color: '#D48700' },
  failed: { label: '失败', color: '#CF1322' },
}

const mockDocuments = [
  { filename: '产品需求规格说明书 v3.2.pdf', file_type: 'PDF', file_size: '2.4 MB', chunk_count: 156, status: 'completed', uploaded_at: '2024-01-15' },
  { filename: 'API 接口设计文档.docx', file_type: 'DOCX', file_size: '1.8 MB', chunk_count: 89, status: 'completed', uploaded_at: '2024-01-14' },
  { filename: '技术架构说明.md', file_type: 'MD', file_size: '245 KB', chunk_count: 34, status: 'processing', uploaded_at: '2024-01-14' },
  { filename: '部署运维手册.txt', file_type: 'TXT', file_size: '128 KB', chunk_count: 0, status: 'failed', uploaded_at: '2024-01-13' },
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

export default function KnowledgeBasesPage() {
  return (
    <div className="flex flex-col gap-6">

      {/* 标题区 */}
      <div className="flex items-end justify-between">
        <div className="flex flex-col gap-2">
          <h1 className="text-[22px] font-bold tracking-tight text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px' }}>
            知识库管理
          </h1>
          <p className="font-mono text-xs text-[#999999]">管理企业私有知识库，增强 AI 助手回答能力</p>
        </div>
        <div className="flex gap-2">
          <button className="flex items-center gap-2 bg-[#0A6B3A] px-4 py-2.5 font-mono text-[9px] font-semibold text-white">
            <Plus className="h-3 w-3" />
            新建知识库
          </button>
        </div>
      </div>

      {/* 知识库卡片 — 3 列 grid */}
      <div className="grid grid-cols-3 gap-4">
        {mockKBs.map((kb) => (
          <div key={kb.name} className="flex flex-col gap-4 bg-white p-5" style={{ border: '1px solid #E8E8E8' }}>
            {/* 标题行 */}
            <div className="flex items-center justify-between">
              <span className="text-sm font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>
                {kb.name}
              </span>
              <StatusTag
                label={kb.enabled ? '已启用' : '已禁用'}
                color={kb.enabled ? '#0A6B3A' : '#999999'}
              />
            </div>
            {/* 描述 */}
            <span className="font-mono text-[10px] font-medium text-[#999999]">{kb.description}</span>
            {/* 统计 */}
            <div className="flex gap-4">
              <div className="flex flex-col gap-1">
                <span className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">文档</span>
                <span className="font-mono text-[14px] font-bold text-[#1A1A1A]">{kb.document_count}</span>
              </div>
              <div className="flex flex-col gap-1">
                <span className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">切片</span>
                <span className="font-mono text-[14px] font-bold text-[#1A1A1A]">{kb.chunk_count.toLocaleString()}</span>
              </div>
            </div>
            {/* 访问权限 */}
            <span className="font-mono text-[10px] font-medium text-[#999999]">{kb.access}</span>
            {/* 操作按钮 */}
            <div className="flex gap-2">
              <button className="flex items-center gap-1.5 border border-[#E8E8E8] bg-white px-3 py-1.5 font-mono text-[9px] font-semibold text-[#1A1A1A]">
                <Upload className="h-3 w-3 text-[#6a6a6a]" />
                上传文档
              </button>
              <button className="flex items-center gap-1.5 border border-[#E8E8E8] bg-white px-3 py-1.5 font-mono text-[9px] font-semibold text-[#1A1A1A]">
                <Search className="h-3 w-3 text-[#6a6a6a]" />
                检索测试
              </button>
            </div>
          </div>
        ))}
      </div>

      {/* 文档列表区 */}
      <div className="flex items-center justify-between">
        <div className="flex flex-col gap-1.5">
          <span className="text-base font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>
            产品文档库 — 文档管理
          </span>
        </div>
        <span className="font-mono text-[10px] font-medium text-[#999999]">42 个文档</span>
      </div>

      {/* 文档表格 */}
      <div className="bg-white" style={{ border: '1px solid #E8E8E8' }}>
        {/* 表头 */}
        <div className="grid grid-cols-6 px-4 py-2.5" style={{ borderBottom: '1px solid #E8E8E8' }}>
          {['文件名', '类型', '大小', '切片', '状态', '上传时间'].map((h) => (
            <span key={h} className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{h}</span>
          ))}
        </div>

        {/* 数据行 */}
        {mockDocuments.map((d, i) => {
          const ft = fileTypeMap[d.file_type]
          const st = docStatusMap[d.status]
          return (
            <div
              key={i}
              className="grid grid-cols-6 items-center px-4 py-3"
              style={{ borderBottom: i < mockDocuments.length - 1 ? '1px solid #E8E8E8' : 'none' }}
            >
              <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{d.filename}</span>
              <div><StatusTag label={ft.label} color={ft.color} /></div>
              <span className="font-mono text-[10px] font-medium text-[#999999]">{d.file_size}</span>
              <span className="font-mono text-[10px] font-medium text-[#999999]">{d.chunk_count}</span>
              <div><StatusTag label={st.label} color={st.color} /></div>
              <span className="font-mono text-[10px] font-medium text-[#999999]">{d.uploaded_at}</span>
            </div>
          )
        })}
      </div>
    </div>
  )
}
