import { Plus, ChevronRight, Building2, Folder, Coins, KeyRound, Save, Trash2, Loader2 } from 'lucide-react'
import { useState, useEffect, useCallback, useMemo } from 'react'
import { api } from '@/lib/api'
import { TablePagination } from '@/components/ui/table-pagination'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible'
import { Dialog, DialogContent } from '@/components/ui/dialog'
import { FormLabel } from '@/components/ui/form-helpers'
import { StatusMessage } from '@/components/ui/form-helpers'

/* ── 类型 ── */

interface Department {
  id: string
  name: string
  description?: string
  parent_id?: string | null
  member_count: number
  token_quota_enabled: boolean
  token_quota_per_day?: number | null
  created_at: string
  updated_at: string
}

interface DepartmentDetail extends Department {
  parent_name?: string | null
  model_whitelist_count?: number
}

interface DeptMember {
  id: string
  username: string
  email?: string
  role?: string
  status?: string
  created_at: string
}

interface WhitelistModel {
  id: string
  model_id: string
  display_name: string
  provider: string
  enabled: boolean
}

interface TreeNode {
  dept: Department
  children: TreeNode[]
}

/* ── 树形构建 ── */

function buildTree(departments: Department[]): TreeNode[] {
  const map = new Map<string, TreeNode>()
  const roots: TreeNode[] = []
  for (const dept of departments) map.set(dept.id, { dept, children: [] })
  for (const dept of departments) {
    const node = map.get(dept.id)!
    if (dept.parent_id && map.has(dept.parent_id)) {
      map.get(dept.parent_id)!.children.push(node)
    } else {
      roots.push(node)
    }
  }
  return roots
}

/* ── TreeItem ── */

function TreeItem({ node, depth = 0, selected, onSelect }: {
  node: TreeNode; depth?: number; selected: string; onSelect: (id: string) => void
}) {
  const isSelected = node.dept.id === selected
  const hasChildren = node.children.length > 0
  const isRoot = depth === 0
  const pl = 12 + depth * 24

  const row = (
    <button
      className="flex w-full items-center gap-2 text-left"
      style={{
        padding: `8px 12px 8px ${pl}px`,
        backgroundColor: isSelected ? '#0A6B3A10' : 'transparent',
        borderLeft: isSelected ? '2px solid #0A6B3A' : '2px solid transparent',
      }}
      onClick={() => onSelect(node.dept.id)}
    >
      {isRoot ? <Building2 className="h-3 w-3 shrink-0 text-[#0A6B3A]" /> : <Folder className="h-3 w-3 shrink-0 text-[#6a6a6a]" />}
      <span className="flex-1 font-mono text-[10px]" style={{ color: isSelected ? '#1A1A1A' : '#333', fontWeight: isSelected ? 600 : 500 }}>{node.dept.name}</span>
      <span className="font-mono text-[9px]" style={{ color: '#999' }}>{node.dept.member_count}人</span>
    </button>
  )

  if (!hasChildren) return row

  return (
    <Collapsible defaultOpen>
      <div className="flex w-full items-center" style={{ padding: `8px 12px 8px ${pl}px`, backgroundColor: isSelected ? '#0A6B3A10' : 'transparent', borderLeft: isSelected ? '2px solid #0A6B3A' : '2px solid transparent' }}>
        <CollapsibleTrigger className="mr-1 flex shrink-0 items-center justify-center">
          <ChevronRight className="h-2.5 w-2.5 text-[#6a6a6a] transition-transform [[data-state=open]>&]:rotate-90" />
        </CollapsibleTrigger>
        <button className="flex flex-1 items-center gap-2 text-left" onClick={() => onSelect(node.dept.id)}>
          {isRoot ? <Building2 className="h-3 w-3 shrink-0 text-[#0A6B3A]" /> : <Folder className="h-3 w-3 shrink-0 text-[#6a6a6a]" />}
          <span className="flex-1 font-mono text-[10px]" style={{ color: isSelected ? '#1A1A1A' : '#333', fontWeight: isSelected ? 600 : 500 }}>{node.dept.name}</span>
          <span className="font-mono text-[9px] text-[#999]">{node.dept.member_count}人</span>
        </button>
      </div>
      <CollapsibleContent>
        {node.children.map(c => <TreeItem key={c.dept.id} node={c} depth={depth + 1} selected={selected} onSelect={onSelect} />)}
      </CollapsibleContent>
    </Collapsible>
  )
}

/* ── 创建部门弹窗 ── */

function CreateDeptDialog({ open, departments, onClose, onCreated }: {
  open: boolean; departments: Department[]; onClose: () => void; onCreated: () => void
}) {
  const [name, setName] = useState('')
  const [description, setDescription] = useState('')
  const [parentId, setParentId] = useState('')
  const [quotaEnabled, setQuotaEnabled] = useState(false)
  const [quotaPerDay, setQuotaPerDay] = useState('')
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState('')

  const reset = () => { setName(''); setDescription(''); setParentId(''); setQuotaEnabled(false); setQuotaPerDay(''); setError('') }

  const handleSave = async () => {
    if (name.trim().length < 2) { setError('部门名称至少 2 个字符'); return }
    setSaving(true); setError('')
    try {
      await api.post('/departments', {
        name: name.trim(),
        description: description || null,
        parent_id: parentId || null,
        token_quota_enabled: quotaEnabled,
        token_quota_per_day: quotaEnabled ? Number(quotaPerDay) || null : null,
      })
      reset(); onCreated()
    } catch (err: any) {
      setError(err.response?.data?.details || err.response?.data?.error || '创建失败')
    } finally { setSaving(false) }
  }

  return (
    <Dialog open={open} onOpenChange={(v) => { if (!v) { reset(); onClose() } }}>
      <DialogContent className="sm:max-w-[520px] rounded-none p-0 gap-0 ring-0" style={{ border: '1px solid #E8E8E8' }} showCloseButton={false}>
        {error && <StatusMessage type="error" message={error} />}
        <div className="flex items-center justify-between px-6 py-5" style={{ borderBottom: '1px solid #E8E8E8' }}>
          <span className="font-mono text-xs font-semibold text-[#1A1A1A]">创建部门</span>
        </div>
        <div className="flex flex-col gap-4 px-6 py-5">
          <div className="flex flex-col gap-1.5">
            <FormLabel required>部门名称</FormLabel>
            <input value={name} onChange={e => setName(e.target.value)} placeholder="输入部门名称" className="border border-[#E8E8E8] bg-[#F5F5F5] px-3 py-2 font-mono text-[10px] text-[#1A1A1A] outline-none focus:border-[#0A6B3A]" />
          </div>
          <div className="flex flex-col gap-1.5">
            <FormLabel>上级部门</FormLabel>
            <select value={parentId} onChange={e => setParentId(e.target.value)} className="border border-[#E8E8E8] bg-[#F5F5F5] px-3 py-2 font-mono text-[10px] text-[#1A1A1A] outline-none">
              <option value="">无（顶级部门）</option>
              {departments.map(d => <option key={d.id} value={d.id}>{d.name}</option>)}
            </select>
          </div>
          <div className="flex flex-col gap-1.5">
            <FormLabel>描述</FormLabel>
            <textarea value={description} onChange={e => setDescription(e.target.value)} placeholder="输入部门描述（可选）" rows={2} className="border border-[#E8E8E8] bg-[#F5F5F5] px-3 py-2 font-mono text-[10px] text-[#1A1A1A] outline-none resize-none" />
          </div>
          <div className="flex items-center gap-3">
            <FormLabel>启用 Token 限额</FormLabel>
            <button onClick={() => setQuotaEnabled(!quotaEnabled)} className={`h-5 w-9 rounded-full transition-colors ${quotaEnabled ? 'bg-[#0A6B3A]' : 'bg-[#D9D9D9]'}`}>
              <div className={`h-4 w-4 rounded-full bg-white shadow transition-transform ${quotaEnabled ? 'translate-x-4' : 'translate-x-0.5'}`} />
            </button>
          </div>
          {quotaEnabled && (
            <div className="flex flex-col gap-1.5">
              <FormLabel required>每日 Token 限额</FormLabel>
              <input type="number" value={quotaPerDay} onChange={e => setQuotaPerDay(e.target.value)} min={0} placeholder="输入限额" className="border border-[#E8E8E8] bg-[#F5F5F5] px-3 py-2 font-mono text-[10px] text-[#1A1A1A] outline-none" />
            </div>
          )}
        </div>
        <div className="flex justify-end gap-2 px-6 py-4" style={{ borderTop: '1px solid #E8E8E8' }}>
          <button onClick={() => { reset(); onClose() }} className="border border-[#E8E8E8] bg-white px-4 py-2 font-mono text-[9px] font-semibold text-[#1A1A1A]">取消</button>
          <button onClick={handleSave} disabled={saving} className="flex items-center gap-1.5 bg-[#0A6B3A] px-4 py-2 font-mono text-[9px] font-semibold text-white disabled:opacity-50">
            {saving ? <Loader2 className="h-3 w-3 animate-spin" /> : <Plus className="h-3 w-3" />}
            创建
          </button>
        </div>
      </DialogContent>
    </Dialog>
  )
}

/* ── 配额卡片（接入 API） ── */

function QuotaCard({ dept, onSaved }: { dept: DepartmentDetail; onSaved: () => void }) {
  const [quotaEnabled, setQuotaEnabled] = useState(dept.token_quota_enabled)
  const [quotaPerDay, setQuotaPerDay] = useState(String(dept.token_quota_per_day ?? ''))
  const [saving, setSaving] = useState(false)
  const [msg, setMsg] = useState<{ type: 'success' | 'error'; text: string } | null>(null)

  useEffect(() => {
    setQuotaEnabled(dept.token_quota_enabled)
    setQuotaPerDay(String(dept.token_quota_per_day ?? ''))
    setMsg(null)
  }, [dept])

  const handleSave = async () => {
    setSaving(true); setMsg(null)
    try {
      await api.put(`/departments/${dept.id}`, {
        token_quota_enabled: quotaEnabled,
        token_quota_per_day: quotaEnabled ? (Number(quotaPerDay) || null) : null,
      })
      setMsg({ type: 'success', text: '配额已保存' })
      onSaved()
    } catch (err: any) {
      setMsg({ type: 'error', text: err.response?.data?.error || '保存失败' })
    } finally { setSaving(false) }
  }

  const handleReset = () => {
    setQuotaEnabled(dept.token_quota_enabled)
    setQuotaPerDay(String(dept.token_quota_per_day ?? ''))
    setMsg(null)
  }

  return (
    <div className="flex flex-1 flex-col gap-3 bg-white p-5" style={{ border: '1px solid #E8E8E8' }}>
      <div className="flex items-center gap-2.5 pb-3" style={{ borderBottom: '1px solid #E8E8E8' }}>
        <Coins className="h-3.5 w-3.5 text-[#0A6B3A]" />
        <span className="font-mono text-[11px] font-semibold tracking-[0.5px] text-[#1A1A1A]">Token 配额设置</span>
      </div>
      {msg && <StatusMessage type={msg.type} message={msg.text} />}
      <div className="flex items-center justify-between">
        <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">启用限额</span>
        <button onClick={() => setQuotaEnabled(!quotaEnabled)} className={`h-5 w-9 rounded-full transition-colors ${quotaEnabled ? 'bg-[#0A6B3A]' : 'bg-[#D9D9D9]'}`}>
          <div className={`h-4 w-4 rounded-full bg-white shadow transition-transform ${quotaEnabled ? 'translate-x-4' : 'translate-x-0.5'}`} />
        </button>
      </div>
      {quotaEnabled && (
        <div className="flex items-center justify-between">
          <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">日配额</span>
          <div className="flex items-center gap-2 bg-[#F5F5F5] px-2.5 py-1.5" style={{ border: '1px solid #E8E8E8' }}>
            <input type="number" value={quotaPerDay} onChange={e => setQuotaPerDay(e.target.value)} min={0} className="w-24 bg-transparent font-mono text-[10px] text-[#1A1A1A] outline-none" />
            <span className="font-mono text-[9px] text-[#999]">Token</span>
          </div>
        </div>
      )}
      <div className="flex justify-end gap-2 pt-2" style={{ borderTop: '1px solid #E8E8E8' }}>
        <button onClick={handleReset} className="border border-[#E8E8E8] bg-white px-4 py-2 font-mono text-[9px] font-semibold text-[#1A1A1A]">重置</button>
        <button onClick={handleSave} disabled={saving} className="flex items-center gap-1.5 bg-[#0A6B3A] px-4 py-2 font-mono text-[9px] font-semibold text-white disabled:opacity-50">
          {saving ? <Loader2 className="h-2.5 w-2.5 animate-spin" /> : <Save className="h-2.5 w-2.5" />}
          保存配额
        </button>
      </div>
    </div>
  )
}

/* ── 白名单卡片（接入 API） ── */

function AccessCard({ whitelist }: { whitelist: WhitelistModel[] }) {
  return (
    <div className="flex flex-1 flex-col gap-3 bg-white p-5" style={{ border: '1px solid #E8E8E8' }}>
      <div className="flex items-center gap-2.5 pb-3" style={{ borderBottom: '1px solid #E8E8E8' }}>
        <KeyRound className="h-3.5 w-3.5 text-[#0A6B3A]" />
        <span className="font-mono text-[11px] font-semibold tracking-[0.5px] text-[#1A1A1A]">权限与白名单配置</span>
      </div>
      <div className="flex items-center justify-between">
        <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">可用模型白名单</span>
        <div className="flex items-center gap-2">
          <span className="font-mono text-[10px] font-medium text-[#0A6B3A]">
            {whitelist.length > 0 ? `${whitelist.length} 个模型` : '全部可用'}
          </span>
        </div>
      </div>
      {whitelist.length > 0 && (
        <div className="flex flex-wrap gap-1.5">
          {whitelist.map(m => (
            <span key={m.id} className="bg-[#F5F5F5] px-2 py-1 font-mono text-[9px] text-[#1A1A1A]" style={{ border: '1px solid #E8E8E8' }}>
              {m.display_name}
            </span>
          ))}
        </div>
      )}
    </div>
  )
}

/* ── 成员表格（接入 API） ── */

const memberColumns = [
  { key: 'username', label: '用户名', width: 'flex-1' },
  { key: 'role', label: '角色', width: 'w-[100px]' },
  { key: 'email', label: '邮箱', width: 'w-[160px]' },
  { key: 'status', label: '状态', width: 'w-[60px]' },
] as const

function MemberTable({ dept, members, onDelete }: {
  dept: DepartmentDetail; members: DeptMember[]; onDelete: () => void
}) {
  const [currentPage, setCurrentPage] = useState(1)
  const [confirmDelete, setConfirmDelete] = useState(false)
  const [deleting, setDeleting] = useState(false)
  const [deleteError, setDeleteError] = useState('')

  const canDelete = dept.member_count === 0

  const handleDelete = async () => {
    setDeleting(true); setDeleteError('')
    try {
      await api.delete(`/departments/${dept.id}`)
      setConfirmDelete(false)
      onDelete()
    } catch (err: any) {
      setDeleteError(err.response?.data?.details || err.response?.data?.error || '删除失败')
    } finally { setDeleting(false) }
  }

  const pageSize = 10
  const paged = members.slice((currentPage - 1) * pageSize, currentPage * pageSize)

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <div className="flex flex-col gap-1">
          <span className="font-mono text-[9px] font-medium text-[#0A6B3A]">// 部门成员</span>
          <span className="text-sm font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>{dept.name}</span>
        </div>
        <div className="flex items-center gap-3">
          <span className="font-mono text-[10px] font-medium text-[#999]">{dept.member_count} 人</span>
          <button
            onClick={() => { setConfirmDelete(true); setDeleteError('') }}
            disabled={!canDelete}
            className="flex items-center gap-1.5 border border-[#E8E8E8] px-3 py-1.5 font-mono text-[9px] font-semibold text-[#CF1322] disabled:opacity-30"
            title={canDelete ? '删除部门' : '有成员或子部门时无法删除'}
          >
            <Trash2 className="h-3 w-3" />
            删除部门
          </button>
        </div>
      </div>

      {deleteError && <StatusMessage type="error" message={deleteError} />}

      {/* 删除确认 */}
      <Dialog open={confirmDelete} onOpenChange={setConfirmDelete}>
        <DialogContent className="sm:max-w-[400px] rounded-none p-0 gap-0 ring-0" style={{ border: '1px solid #E8E8E8' }} showCloseButton={false}>
          <div className="px-6 py-5">
            <p className="font-mono text-xs text-[#1A1A1A]">确定要删除部门 <strong>{dept.name}</strong> 吗？此操作不可撤销。</p>
          </div>
          <div className="flex justify-end gap-2 px-6 py-4" style={{ borderTop: '1px solid #E8E8E8' }}>
            <button onClick={() => setConfirmDelete(false)} className="border border-[#E8E8E8] bg-white px-4 py-2 font-mono text-[9px] font-semibold text-[#1A1A1A]">取消</button>
            <button onClick={handleDelete} disabled={deleting} className="flex items-center gap-1.5 bg-[#CF1322] px-4 py-2 font-mono text-[9px] font-semibold text-white disabled:opacity-50">
              {deleting ? <Loader2 className="h-3 w-3 animate-spin" /> : <Trash2 className="h-3 w-3" />}
              确认删除
            </button>
          </div>
        </DialogContent>
      </Dialog>

      <div className="bg-white" style={{ border: '1px solid #E8E8E8' }}>
        <div className="flex items-center px-4 py-2.5" style={{ borderBottom: '1px solid #E8E8E8' }}>
          {memberColumns.map(col => (
            <span key={col.key} className={`font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999] ${col.width}`}>{col.label}</span>
          ))}
        </div>
        {paged.length === 0 ? (
          <div className="px-4 py-6 text-center font-mono text-[10px] text-[#999]">暂无成员</div>
        ) : paged.map((m, i) => (
          <div key={m.id} className="flex items-center px-4 py-2.5" style={{ borderBottom: i < paged.length - 1 ? '1px solid #E8E8E8' : 'none' }}>
            <span className="flex-1 font-mono text-[10px] font-medium text-[#1A1A1A]">{m.username}</span>
            <span className="w-[100px] font-mono text-[10px] font-medium" style={{ color: m.role === '管理员' ? '#0A6B3A' : '#999' }}>{m.role || '-'}</span>
            <span className="w-[160px] font-mono text-[10px] font-medium text-[#1A1A1A] truncate">{m.email || '-'}</span>
            <div className="flex w-[60px] items-center gap-1.5">
              <div className={`h-1.5 w-1.5 rounded-full ${m.status === 'active' ? 'bg-[#0A6B3A]' : 'bg-[#999]'}`} />
              <span className={`font-mono text-[9px] font-semibold ${m.status === 'active' ? 'text-[#0A6B3A]' : 'text-[#999]'}`}>
                {m.status === 'active' ? '在线' : '离线'}
              </span>
            </div>
          </div>
        ))}
        {members.length > pageSize && <TablePagination current={currentPage} total={members.length} pageSize={pageSize} onChange={setCurrentPage} />}
      </div>
    </div>
  )
}

/* ── 主组件 ── */

export default function DepartmentsPage() {
  const [departments, setDepartments] = useState<Department[]>([])
  const [loading, setLoading] = useState(true)
  const [selectedId, setSelectedId] = useState('')
  const [createOpen, setCreateOpen] = useState(false)

  // 选中部门的详情数据
  const [detail, setDetail] = useState<DepartmentDetail | null>(null)
  const [members, setMembers] = useState<DeptMember[]>([])
  const [whitelist, setWhitelist] = useState<WhitelistModel[]>([])
  const [detailLoading, setDetailLoading] = useState(false)

  const loadDepartments = useCallback(async () => {
    setLoading(true)
    try {
      const res = await api.get('/departments')
      setDepartments(res.data.departments || [])
    } catch (err) {
      console.error('加载部门列表失败', err)
    } finally { setLoading(false) }
  }, [])

  const loadDetail = useCallback(async (id: string) => {
    setDetailLoading(true)
    try {
      const [dRes, mRes, wRes] = await Promise.all([
        api.get(`/departments/${id}`),
        api.get(`/departments/${id}/members`),
        api.get(`/departments/${id}/model-whitelist`),
      ])
      setDetail(dRes.data)
      setMembers(mRes.data.members || [])
      setWhitelist(wRes.data.models || [])
    } catch (err) {
      console.error('加载部门详情失败', err)
    } finally { setDetailLoading(false) }
  }, [])

  useEffect(() => { loadDepartments() }, [loadDepartments])

  // 选中部门变化时加载详情
  useEffect(() => {
    if (selectedId) loadDetail(selectedId)
    else { setDetail(null); setMembers([]); setWhitelist([]) }
  }, [selectedId, loadDetail])

  // 加载完成后默认选中第一个
  useEffect(() => {
    if (departments.length > 0 && !selectedId) setSelectedId(departments[0].id)
  }, [departments, selectedId])

  const tree = useMemo(() => buildTree(departments), [departments])

  const handleCreated = () => { setCreateOpen(false); loadDepartments() }
  const handleDeleted = () => { setSelectedId(''); loadDepartments() }
  const handleQuotaSaved = () => { loadDepartments(); if (selectedId) loadDetail(selectedId) }

  return (
    <div className="flex flex-col gap-6">
      {/* 标题 + 新建按钮 */}
      <div className="flex items-end justify-between">
        <div className="flex flex-col gap-2">
          <h1 className="text-[22px] font-bold tracking-tight text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px' }}>
            部门管理
          </h1>
          <p className="font-mono text-xs text-[#999]">组织架构、Token 配额、知识库权限与模型白名单管理</p>
        </div>
        <button onClick={() => setCreateOpen(true)} className="flex items-center gap-2 bg-[#0A6B3A] px-4 py-2.5 font-mono text-[9px] font-semibold text-white">
          <Plus className="h-3 w-3" />
          新建部门
        </button>
      </div>

      {/* 两列布局 */}
      <div className="flex gap-5">
        {/* 左：树 */}
        <div className="flex w-[320px] shrink-0 flex-col gap-4 bg-white p-5" style={{ border: '1px solid #E8E8E8' }}>
          <div className="flex items-center gap-2.5 pb-4" style={{ borderBottom: '1px solid #E8E8E8' }}>
            <Building2 className="h-3.5 w-3.5 text-[#0A6B3A]" />
            <span className="font-mono text-[11px] font-semibold tracking-[0.5px] text-[#1A1A1A]">组织架构</span>
          </div>
          {loading ? (
            <div className="flex justify-center py-8"><Loader2 className="h-5 w-5 animate-spin text-[#999]" /></div>
          ) : tree.length === 0 ? (
            <div className="py-8 text-center font-mono text-[10px] text-[#999]">暂无部门，点击右上角创建</div>
          ) : (
            <div className="flex flex-col gap-1">
              {tree.map(node => <TreeItem key={node.dept.id} node={node} selected={selectedId} onSelect={setSelectedId} />)}
            </div>
          )}
        </div>

        {/* 右：详情 */}
        <div className="flex min-w-0 flex-1 flex-col gap-4">
          {!selectedId ? (
            <div className="flex items-center justify-center py-32 font-mono text-[11px] text-[#999]">请从左侧选择一个部门查看详情</div>
          ) : detailLoading && !detail ? (
            <div className="flex justify-center py-32"><Loader2 className="h-5 w-5 animate-spin text-[#999]" /></div>
          ) : detail ? (
            <>
              {/* 统计卡片 */}
              <div className="flex gap-3">
                {[
                  { label: '成员数', value: String(detail.member_count), color: '#1A1A1A', border: '#E8E8E8' },
                  { label: 'Token 配额/天', value: detail.token_quota_enabled ? (detail.token_quota_per_day?.toLocaleString() ?? '未设置') : '未启用', color: detail.token_quota_enabled ? '#0A6B3A' : '#999', border: detail.token_quota_enabled ? '#0A6B3A40' : '#E8E8E8' },
                  { label: '模型白名单', value: whitelist.length > 0 ? `${whitelist.length} 个` : '全部', color: '#1A1A1A', border: '#E8E8E8' },
                ].map(c => (
                  <div key={c.label} className="flex flex-1 flex-col gap-2 bg-white p-4" style={{ border: `1px solid ${c.border}` }}>
                    <span className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999]">{c.label}</span>
                    <span className="text-[24px] font-bold tracking-tight" style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px', color: c.color }}>{c.value}</span>
                  </div>
                ))}
              </div>

              {/* 配额 + 白名单 */}
              <div className="flex gap-4">
                <QuotaCard dept={detail} onSaved={handleQuotaSaved} />
                <AccessCard whitelist={whitelist} />
              </div>

              {/* 成员表格 */}
              <MemberTable dept={detail} members={members} onDelete={handleDeleted} />
            </>
          ) : null}
        </div>
      </div>

      <CreateDeptDialog open={createOpen} departments={departments} onClose={() => setCreateOpen(false)} onCreated={handleCreated} />
    </div>
  )
}
