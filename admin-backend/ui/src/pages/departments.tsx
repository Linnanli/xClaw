import { Plus, ChevronDown, ChevronRight } from 'lucide-react'
import { useState } from 'react'
import { TablePagination } from '@/components/ui/table-pagination'
import { StatusTag } from '@/components/ui/status-tag'

/* ── Mock 数据 ── */

const deptStats = [
  { label: '成员数', value: '89', valueColor: '#1A1A1A', borderColor: '#E8E8E8' },
  { label: '月Token消耗', value: '48.7M', valueColor: '#0A6B3A', borderColor: '#E8E8E8' },
  { label: '月配额', value: '100M', valueColor: '#1A1A1A', borderColor: '#E8E8E8' },
  { label: '知识库', value: '3', valueColor: '#1A1A1A', borderColor: '#E8E8E8' },
]

const orgTree = [
  { name: 'IronClaw 总部', level: 0, expanded: true, children: true },
  { name: '研发部', level: 1, expanded: true, children: true },
  { name: '前端组', level: 2, expanded: false, children: false },
  { name: '后端组', level: 2, expanded: false, children: false },
  { name: '测试组', level: 2, expanded: false, children: false },
  { name: '产品部', level: 1, expanded: false, children: false },
  { name: '市场部', level: 1, expanded: false, children: false },
  { name: '运营部', level: 1, expanded: false, children: false },
  { name: '法务部', level: 1, expanded: false, children: false },
  { name: '人力资源部', level: 1, expanded: false, children: false },
]

const statusMap: Record<string, { label: string; color: string }> = {
  active: { label: '活跃', color: '#0A6B3A' },
  disabled: { label: '禁用', color: '#999999' },
}

const mockMembers = [
  { username: 'zhang.wei', role: '开发工程师', department: '前端组', todayToken: '12,500', status: 'active' },
  { username: 'li.ming', role: '高级工程师', department: '后端组', todayToken: '28,400', status: 'active' },
]

/* ── 组件 ── */

export default function DepartmentsPage() {
  const [currentPage, setCurrentPage] = useState(1)

  return (
    <div className="flex flex-col gap-6">

      {/* 标题区 */}
      <div className="flex items-end justify-between">
        <div className="flex flex-col gap-2">
          <h1 className="text-[22px] font-bold tracking-tight text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px' }}>
            部门管理
          </h1>
          <p className="font-mono text-xs text-[#999999]">组织架构、Token 配额、知识库权限与模型白名单管理</p>
        </div>
        <button className="flex items-center gap-2 bg-[#0A6B3A] px-4 py-2.5 font-mono text-[9px] font-semibold text-white">
          <Plus className="h-3 w-3" />
          新建部门
        </button>
      </div>

      {/* 统计卡片 — 4 列 */}
      <div className="grid grid-cols-4 gap-2.5">
        {deptStats.map((s) => (
          <div key={s.label} className="flex flex-col gap-4 bg-white p-4" style={{ border: `1px solid ${s.borderColor}` }}>
            <span className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{s.label}</span>
            <span
              className="text-[28px] font-bold tracking-tight"
              style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px', color: s.valueColor }}
            >
              {s.value}
            </span>
          </div>
        ))}
      </div>

      {/* 2 列布局：组织架构 + 配置 */}
      <div className="grid grid-cols-2 gap-4">
        {/* 左：组织架构树 */}
        <div className="flex flex-col gap-4 bg-white p-5" style={{ border: '1px solid #E8E8E8' }}>
          <span className="text-sm font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>
            组织架构
          </span>
          <div className="flex flex-col gap-0" data-testid="org-tree">
            {orgTree.map((node) => (
              <div
                key={node.name}
                className="flex items-center gap-2 py-2"
                style={{ paddingLeft: node.level * 20 + 4 }}
              >
                {node.children ? (
                  node.expanded ? (
                    <ChevronDown className="h-3 w-3 text-[#999999]" />
                  ) : (
                    <ChevronRight className="h-3 w-3 text-[#999999]" />
                  )
                ) : (
                  <div className="h-3 w-3" />
                )}
                <span
                  className="font-mono text-[10px] font-medium"
                  style={{ color: node.level === 0 ? '#0A6B3A' : '#1A1A1A', fontWeight: node.level === 0 ? 600 : 500 }}
                >
                  {node.name}
                </span>
              </div>
            ))}
          </div>
        </div>

        {/* 右列 */}
        <div className="flex flex-col gap-4">
          {/* Token 配额设置 */}
          <div className="flex flex-col gap-4 bg-white p-5" style={{ border: '1px solid #E8E8E8' }}>
            <span className="text-sm font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>
              Token 配额设置
            </span>
            <div className="flex items-center justify-between">
              <span className="font-mono text-[10px] font-medium text-[#999999]">日配额</span>
              <span className="font-mono text-[10px] font-semibold text-[#1A1A1A]">5,000,000</span>
            </div>
            <div className="flex items-center justify-between">
              <span className="font-mono text-[10px] font-medium text-[#999999]">月配额</span>
              <span className="font-mono text-[10px] font-semibold text-[#1A1A1A]">100,000,000</span>
            </div>
            <div className="flex items-center justify-between">
              <span className="font-mono text-[10px] font-medium text-[#999999]">月预算</span>
              <span className="font-mono text-[10px] font-semibold text-[#1A1A1A]">¥2,400</span>
            </div>
            <div className="flex justify-end gap-2">
              <button className="border border-[#E8E8E8] bg-white px-3 py-1.5 font-mono text-[9px] font-semibold text-[#999999]">重置</button>
              <button className="bg-[#0A6B3A] px-3 py-1.5 font-mono text-[9px] font-semibold text-white">保存</button>
            </div>
          </div>

          {/* 权限与白名单配置 */}
          <div className="flex flex-col gap-4 bg-white p-5" style={{ border: '1px solid #E8E8E8' }}>
            <span className="text-sm font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>
              权限与白名单配置
            </span>
            {[
              { label: '可用知识库', value: '2 个' },
              { label: '可用模型', value: '2 个' },
              { label: '可用技能', value: '全部 12 个' },
            ].map((item) => (
              <div key={item.label} className="flex items-center justify-between">
                <span className="font-mono text-[10px] font-medium text-[#999999]">{item.label}</span>
                <div className="flex items-center gap-2">
                  <span className="font-mono text-[10px] font-semibold text-[#1A1A1A]">{item.value}</span>
                  <button className="font-mono text-[10px] font-semibold text-[#0A6B3A]">编辑</button>
                </div>
              </div>
            ))}
            <div className="flex justify-end gap-2">
              <button className="border border-[#E8E8E8] bg-white px-3 py-1.5 font-mono text-[9px] font-semibold text-[#999999]">取消</button>
              <button className="bg-[#0A6B3A] px-3 py-1.5 font-mono text-[9px] font-semibold text-white">保存</button>
            </div>
          </div>
        </div>
      </div>

      {/* 部门成员表格 */}
      <div className="bg-white" style={{ border: '1px solid #E8E8E8' }}>
        <div className="px-4 py-3" style={{ borderBottom: '1px solid #E8E8E8' }}>
          <span className="text-sm font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>
            部门成员
          </span>
        </div>
        <div className="grid grid-cols-5 px-4 py-2.5" style={{ borderBottom: '1px solid #E8E8E8' }}>
          {['用户名', '角色', '部门', '今日Token', '状态'].map((h) => (
            <span key={h} className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{h}</span>
          ))}
        </div>
        {mockMembers.map((m, i) => {
          const st = statusMap[m.status]
          return (
            <div
              key={m.username}
              className="grid grid-cols-5 items-center px-4 py-3"
              style={{ borderBottom: i < mockMembers.length - 1 ? '1px solid #E8E8E8' : 'none' }}
            >
              <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{m.username}</span>
              <span className="font-mono text-[10px] font-medium text-[#999999]">{m.role}</span>
              <span className="font-mono text-[10px] font-medium text-[#999999]">{m.department}</span>
              <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{m.todayToken}</span>
              <div><StatusTag label={st.label} color={st.color} /></div>
            </div>
          )
        })}
        <TablePagination current={currentPage} total={24} pageSize={10} onChange={setCurrentPage} />
      </div>
    </div>
  )
}
