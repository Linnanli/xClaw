import { Plus, ChevronRight, Building2, Folder, Coins, KeyRound, Save, Pencil } from 'lucide-react'
import { useState, useEffect, useCallback, useMemo } from 'react'
import { api } from '@/lib/api'
import { TablePagination } from '@/components/ui/table-pagination'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible'

/* ── 类型 ── */

interface Department {
  id: string
  name: string
  description?: string
  member_count: number
  token_quota_enabled: boolean
  token_quota_per_day?: number
  created_at: string
}

interface TreeNode {
  name: string
  count: number
  icon: 'building' | 'folder'
  defaultOpen?: boolean
  children?: TreeNode[]
}

/* ── 从 API 数据构建组织架构树 ── */

function buildOrgTree(departments: Department[]): TreeNode[] {
  const children: TreeNode[] = departments.map((d) => ({
    name: d.name,
    count: d.member_count,
    icon: 'folder' as const,
  }))
  const totalCount = departments.reduce((sum, d) => sum + d.member_count, 0)
  return [{
    name: 'IronClaw 总部',
    count: totalCount,
    icon: 'building' as const,
    defaultOpen: true,
    children,
  }]
}

/* ── 配额/权限 mock（API 无对应端点） ── */

const quotaRows = [
  { label: '日配额', value: '5,000,000', unit: 'Token' },
  { label: '月配额', value: '100,000,000', unit: 'Token' },
  { label: '月预算', value: '¥2,400', unit: '' },
]

const accessRows = [
  { label: '可用知识库', value: '2 个知识库', valueColor: '#0A6B3A', editColor: '#0A6B3A', borderColor: '#0A6B3A' },
  { label: '可用模型白名单', value: '2 个模型', valueColor: '#0A6B3A', editColor: '#0A6B3A', borderColor: '#0A6B3A' },
  { label: '可用技能', value: '全部 (12 个)', valueColor: '#999999', editColor: '#999999', borderColor: '#E8E8E8' },
]

/* ── 部门成员 mock（API 无成员列表端点） ── */

const mockMembers = [
  { username: 'zhang.wei', role: '管理员', roleColor: '#0A6B3A', department: '研发部', todayToken: '42,180', online: true },
  { username: 'li.ming', role: '普通用户', roleColor: '#999999', department: '产品部', todayToken: '18,450', online: true },
]

/* ── 子组件：TreeItem ── */

function TreeItem({ node, depth = 0, selected, onSelect }: {
  node: TreeNode; depth?: number; selected: string; onSelect: (name: string) => void
}) {
  const isSelected = node.name === selected
  const hasChildren = node.children && node.children.length > 0
  const isRoot = depth === 0
  const paddingLeft = depth === 0 ? 12 : depth === 1 ? 36 : 60
  const labelColor = isRoot ? '#1A1A1A' : isSelected ? '#1A1A1A' : depth >= 2 ? '#999999' : '#1A1A1A'
  const countColor = isSelected ? '#0A6B3A' : depth >= 2 ? '#CCCCCC' : '#999999'
  const rowStyle = {
    padding: `8px 12px 8px ${paddingLeft}px`,
    backgroundColor: isSelected ? '#0A6B3A10' : 'transparent',
    borderLeft: isSelected ? '2px solid #0A6B3A' : '2px solid transparent',
  }

  if (!hasChildren) {
    return (
      <button className="flex w-full items-center gap-2 text-left" style={rowStyle} onClick={() => onSelect(node.name)} data-testid={`tree-node-${node.name}`}>
        {isRoot ? <Building2 className="h-3 w-3 shrink-0 text-[#0A6B3A]" /> : <Folder className="h-3 w-3 shrink-0 text-[#6a6a6a]" />}
        <span className="font-mono text-[10px]" style={{ color: labelColor, fontWeight: isRoot || isSelected ? 600 : 500 }}>{node.name}</span>
        <span className="font-mono text-[9px]" style={{ color: countColor, fontWeight: 500 }}>{node.count}</span>
      </button>
    )
  }

  return (
    <Collapsible defaultOpen={node.defaultOpen}>
      <div className="flex w-full items-center" style={rowStyle}>
        <CollapsibleTrigger className="mr-1 flex shrink-0 items-center justify-center" data-testid={`tree-toggle-${node.name}`}>
          <ChevronRight className="h-2.5 w-2.5 text-[#6a6a6a] transition-transform [[data-state=open]>&]:rotate-90" />
        </CollapsibleTrigger>
        <button className="flex flex-1 items-center gap-2 text-left" onClick={() => onSelect(node.name)} data-testid={`tree-node-${node.name}`}>
          {isRoot ? <Building2 className="h-3 w-3 shrink-0 text-[#0A6B3A]" /> : <Folder className="h-3 w-3 shrink-0 text-[#6a6a6a]" />}
          <span className="font-mono text-[10px]" style={{ color: labelColor, fontWeight: isRoot || isSelected ? 600 : 500 }}>{node.name}</span>
          <span className="font-mono text-[9px]" style={{ color: countColor, fontWeight: 500 }}>{node.count}</span>
        </button>
      </div>
      <CollapsibleContent>
        {node.children!.map((child) => (
          <TreeItem key={child.name} node={child} depth={depth + 1} selected={selected} onSelect={onSelect} />
        ))}
      </CollapsibleContent>
    </Collapsible>
  )
}

/* ── 主组件 ── */

export default function DepartmentsPage() {
  const [currentPage, setCurrentPage] = useState(1)
  const [selectedDept, setSelectedDept] = useState('')
  const [departments, setDepartments] = useState<Department[]>([])
  const [loading, setLoading] = useState(true)

  const loadDepartments = useCallback(async () => {
    setLoading(true)
    try {
      const res = await api.get('/departments')
      setDepartments(res.data.departments || [])
    } catch (err) {
      console.error('加载部门列表失败', err)
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => { loadDepartments() }, [loadDepartments])

  // 加载完成后默认选中第一个部门
  useEffect(() => {
    if (departments.length > 0 && !selectedDept) {
      setSelectedDept(departments[0].name)
    }
  }, [departments, selectedDept])

  const totalMembers = useMemo(() => departments.reduce((sum, d) => sum + d.member_count, 0), [departments])
  const orgTree = useMemo(() => buildOrgTree(departments), [departments])

  const detailCards = [
    { label: '成员数', value: loading ? '—' : String(totalMembers), valueColor: '#1A1A1A', borderColor: '#E8E8E8' },
    { label: '月 Token 消耗', value: '48.7M', valueColor: '#0A6B3A', borderColor: '#0A6B3A40' },
    { label: '月配额', value: '100M', valueColor: '#1A1A1A', borderColor: '#E8E8E8' },
    { label: '知识库', value: '3', valueColor: '#1A1A1A', borderColor: '#E8E8E8' },
  ]

  return (
    <div className="flex flex-col gap-6">
      {/* 标题区 + 新建按钮 */}
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

      {/* 两列布局：树面板 + 详情面板 */}
      <div className="flex gap-5">
        {/* 左：组织架构树（保持 mock） */}
        <div className="flex w-[320px] shrink-0 flex-col gap-4 bg-white p-5" style={{ border: '1px solid #E8E8E8' }} data-testid="org-tree">
          <div className="flex items-center gap-2.5 pb-4" style={{ borderBottom: '1px solid #E8E8E8' }}>
            <Building2 className="h-3.5 w-3.5 text-[#0A6B3A]" />
            <span className="font-mono text-[11px] font-semibold tracking-[0.5px] text-[#1A1A1A]">组织架构</span>
          </div>
          <div className="flex flex-col gap-1">
            {orgTree.map((node) => (
              <TreeItem key={node.name} node={node} selected={selectedDept} onSelect={setSelectedDept} />
            ))}
          </div>
        </div>

        {/* 右：详情面板 */}
        <div className="flex min-w-0 flex-1 flex-col gap-4">
          {/* 统计卡片 — 4 列（成员数从 API 动态计算） */}
          <div className="flex gap-3">
            {detailCards.map((c) => (
              <div key={c.label} className="flex flex-1 flex-col gap-2 bg-white p-4" style={{ border: `1px solid ${c.borderColor}` }}>
                <span className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{c.label}</span>
                <span className="text-[24px] font-bold tracking-tight" style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px', color: c.valueColor }}>
                  {c.value}
                </span>
              </div>
            ))}
          </div>

          {/* 配置卡片 — 2 列（保持 mock） */}
          <div className="flex gap-4">
            <QuotaCard />
            <AccessCard />
          </div>

          {/* 部门成员表格（保持 mock） */}
          <MemberTable currentPage={currentPage} onPageChange={setCurrentPage} />
        </div>
      </div>
    </div>
  )
}

/* ── Token 配额卡片（保持 mock） ── */

function QuotaCard() {
  return (
    <div className="flex flex-1 flex-col gap-3 bg-white p-5" style={{ border: '1px solid #E8E8E8' }}>
      <div className="flex items-center gap-2.5 pb-3" style={{ borderBottom: '1px solid #E8E8E8' }}>
        <Coins className="h-3.5 w-3.5 text-[#0A6B3A]" />
        <span className="font-mono text-[11px] font-semibold tracking-[0.5px] text-[#1A1A1A]">Token 配额设置</span>
      </div>
      {quotaRows.map((r) => (
        <div key={r.label} className="flex items-center justify-between">
          <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{r.label}</span>
          <div className="flex items-center gap-2 bg-[#F5F5F5] px-2.5 py-1.5" style={{ border: '1px solid #E8E8E8' }}>
            <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{r.value}</span>
            {r.unit && <span className="font-mono text-[9px] text-[#999999]">{r.unit}</span>}
          </div>
        </div>
      ))}
      <div className="flex justify-end gap-2 pt-2" style={{ borderTop: '1px solid #E8E8E8' }}>
        <button className="flex items-center justify-center border border-[#E8E8E8] bg-white px-4 py-2 font-mono text-[9px] font-semibold text-[#1A1A1A]">重置</button>
        <button className="flex items-center gap-1.5 bg-[#0A6B3A] px-4 py-2 font-mono text-[9px] font-semibold text-white">
          <Save className="h-2.5 w-2.5" />
          保存配额
        </button>
      </div>
    </div>
  )
}

/* ── 权限与白名单卡片（保持 mock） ── */

function AccessCard() {
  return (
    <div className="flex flex-1 flex-col gap-3 bg-white p-5" style={{ border: '1px solid #E8E8E8' }}>
      <div className="flex items-center gap-2.5 pb-3" style={{ borderBottom: '1px solid #E8E8E8' }}>
        <KeyRound className="h-3.5 w-3.5 text-[#0A6B3A]" />
        <span className="font-mono text-[11px] font-semibold tracking-[0.5px] text-[#1A1A1A]">权限与白名单配置</span>
      </div>
      {accessRows.map((r) => (
        <div key={r.label} className="flex items-center justify-between">
          <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{r.label}</span>
          <div className="flex items-center gap-2">
            <span className="font-mono text-[10px] font-medium" style={{ color: r.valueColor }}>{r.value}</span>
            <button className="flex items-center gap-1.5 border bg-white px-2 py-0.5 font-mono text-[8px] font-semibold" style={{ color: r.editColor, borderColor: r.borderColor }}>
              <Pencil className="h-2.5 w-2.5" />
              编辑
            </button>
          </div>
        </div>
      ))}
      <div className="flex justify-end gap-2 pt-2" style={{ borderTop: '1px solid #E8E8E8' }}>
        <button className="flex items-center justify-center border border-[#E8E8E8] bg-white px-4 py-2 font-mono text-[9px] font-semibold text-[#1A1A1A]">取消</button>
        <button className="flex items-center gap-1.5 bg-[#0A6B3A] px-4 py-2 font-mono text-[9px] font-semibold text-white">
          <Save className="h-2.5 w-2.5" />
          保存配置
        </button>
      </div>
    </div>
  )
}

/* ── 部门成员表格（保持 mock） ── */

const memberColumns = [
  { key: 'username', label: '用户名', width: 'flex-1' },
  { key: 'role', label: '角色', width: 'w-[100px]' },
  { key: 'department', label: '部门', width: 'w-[100px]' },
  { key: 'todayToken', label: '今日 Token', width: 'w-[100px]' },
  { key: 'status', label: '状态', width: 'w-[60px]' },
] as const

function MemberTable({ currentPage, onPageChange }: { currentPage: number; onPageChange: (p: number) => void }) {
  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <div className="flex flex-col gap-1">
          <span className="font-mono text-[9px] font-medium text-[#0A6B3A]">// 部门成员</span>
          <span className="text-sm font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>研发部 — 部门配置</span>
        </div>
        <span className="font-mono text-[10px] font-medium text-[#999999]">89 人</span>
      </div>

      <div className="bg-white" style={{ border: '1px solid #E8E8E8' }}>
        <div className="flex items-center px-4 py-2.5" style={{ borderBottom: '1px solid #E8E8E8' }}>
          {memberColumns.map((col) => (
            <span key={col.key} className={`font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999] ${col.width}`}>{col.label}</span>
          ))}
        </div>
        {mockMembers.map((m, i) => (
          <div key={m.username} className="flex items-center px-4 py-2.5" style={{ borderBottom: i < mockMembers.length - 1 ? '1px solid #E8E8E8' : 'none' }}>
            <span className="flex-1 font-mono text-[10px] font-medium text-[#1A1A1A]">{m.username}</span>
            <span className="w-[100px] font-mono text-[10px] font-medium" style={{ color: m.roleColor }}>{m.role}</span>
            <span className="w-[100px] font-mono text-[10px] font-medium text-[#1A1A1A]">{m.department}</span>
            <span className="w-[100px] font-mono text-[10px] font-medium text-[#1A1A1A]">{m.todayToken}</span>
            <div className="flex w-[60px] items-center gap-1.5">
              <div className="h-1.5 w-1.5 rounded-full bg-[#0A6B3A]" />
              <span className="font-mono text-[9px] font-semibold text-[#0A6B3A]">在线</span>
            </div>
          </div>
        ))}
        <TablePagination current={currentPage} total={89} pageSize={10} onChange={onPageChange} />
      </div>
    </div>
  )
}
