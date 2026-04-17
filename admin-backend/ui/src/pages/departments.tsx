import { Plus, ChevronRight, Building2, Folder, Coins, KeyRound, Save, Trash2, Loader2, Pencil, Search, Check } from 'lucide-react'
import { useState, useEffect, useCallback, useMemo } from 'react'
import { api } from '@/lib/api'
import { TablePagination } from '@/components/ui/table-pagination'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible'
import { Dialog, DialogContent } from '@/components/ui/dialog'
import { FormLabel } from '@/components/ui/form-helpers'
import { StatusMessage } from '@/components/ui/form-helpers'

/* ── 格式化辅助 ── */

function formatCents(cents: number): string {
  return `¥${(cents / 100).toFixed(2)}`
}

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
  created_at: string
}

interface WhitelistModel {
  id: string
  model_id: string
  display_name: string
  provider: string
  enabled: boolean
}

interface WhitelistSkill {
  skill_id: string
  name: string
}

interface ModelOption {
  id: string
  model_id: string
  display_name: string
  provider: string
  enabled: boolean
}

interface SkillOption {
  id: string
  name: string
  enabled: boolean
  review_status: string
}

interface DepartmentQuotaSummary {
  today_total_used_cents: number
  custom_limit_cents: number | null
  children_limit_sum_cents: number
}

interface RootDepartmentQuotaSummary extends DepartmentQuotaSummary {
  is_custom: boolean
  monthly_budget_cents: number | null
  month_total_used_cents: number
}

interface TreeNode {
  dept: Department
  children: TreeNode[]
}

function defaultCreateParentId(departments: Department[]): string {
  const root = departments.find((dept) => !dept.parent_id)
  return root?.id ?? ''
}

function hasRootDepartment(departments: Department[], excludedDeptId?: string): boolean {
  return departments.some((dept) => !dept.parent_id && dept.id !== excludedDeptId)
}

function canBeRootDepartment(departments: Department[], dept: DepartmentDetail): boolean {
  return !hasRootDepartment(departments, dept.id) || !dept.parent_id
}

function toggleSelection(selected: Set<string>, id: string): Set<string> {
  const next = new Set(selected)
  if (next.has(id)) {
    next.delete(id)
  } else {
    next.add(id)
  }
  return next
}

function departmentDailyQuotaLabel(
  detail: DepartmentDetail,
  quotaSummary: DepartmentQuotaSummary | null,
): string {
  if (!quotaSummary) {
    return '—'
  }

  if (quotaSummary.custom_limit_cents != null) {
    return formatCents(quotaSummary.custom_limit_cents)
  }

  if (!detail.parent_id) {
    return `${formatCents(quotaSummary.children_limit_sum_cents)} (累加)`
  }

  return '未设置'
}

function rootQuotaLimitLabel(
  summary: RootDepartmentQuotaSummary | null,
  isCustom: boolean,
  customLimit: string,
): string {
  if (!summary) {
    return '加载中...'
  }

  if (!isCustom) {
    return `${formatCents(summary.children_limit_sum_cents)} (累加)`
  }

  return customLimit ? formatCents(Number(customLimit)) : '未设置'
}

function mapModelOptions(data: unknown): ModelOption[] {
  if (!Array.isArray(data)) {
    return []
  }

  return data.flatMap((item) => {
    const candidate = item as Partial<ModelOption>
    if (
      typeof candidate.id !== 'string' ||
      typeof candidate.model_id !== 'string' ||
      typeof candidate.display_name !== 'string' ||
      typeof candidate.provider !== 'string'
    ) {
      return []
    }

    return [{
      id: candidate.id,
      model_id: candidate.model_id,
      display_name: candidate.display_name,
      provider: candidate.provider,
      enabled: candidate.enabled !== false,
    }]
  })
}

function mapApprovedSkillOptions(data: unknown): SkillOption[] {
  if (!Array.isArray(data)) {
    return []
  }

  return data.flatMap((item) => {
    const candidate = item as Partial<SkillOption>
    if (
      typeof candidate.id !== 'string' ||
      typeof candidate.name !== 'string' ||
      candidate.enabled !== true ||
      candidate.review_status !== 'approved'
    ) {
      return []
    }

    return [candidate as SkillOption]
  })
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
  const [parentId, setParentId] = useState(defaultCreateParentId(departments))
  const [quotaEnabled, setQuotaEnabled] = useState(false)
  const [quotaPerDay, setQuotaPerDay] = useState('')
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState('')

  const rootDepartmentExists = hasRootDepartment(departments)

  const reset = () => {
    setName('')
    setDescription('')
    setParentId(defaultCreateParentId(departments))
    setQuotaEnabled(false)
    setQuotaPerDay('')
    setError('')
  }

  useEffect(() => {
    if (open) {
      reset()
    }
  }, [open, departments])

  const handleSave = async () => {
    if (name.trim().length < 2) { setError('部门名称至少 2 个字符'); return }
    if (rootDepartmentExists && !parentId) { setError('系统只允许一个顶级部门，请为新部门选择上级部门'); return }
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
              {!rootDepartmentExists && <option value="">无（顶级部门）</option>}
              {departments.map(d => <option key={d.id} value={d.id}>{d.name}</option>)}
            </select>
            {rootDepartmentExists && <p className="font-mono text-[9px] text-[#999]">系统仅允许一个顶级部门，新部门需挂在现有部门下。</p>}
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

/* ── 编辑部门弹窗 ── */

function EditDeptDialog({ open, dept, departments, onClose, onSaved }: {
  open: boolean; dept: DepartmentDetail; departments: Department[]; onClose: () => void; onSaved: () => void
}) {
  const [name, setName] = useState(dept.name)
  const [description, setDescription] = useState(dept.description || '')
  const [parentId, setParentId] = useState(dept.parent_id || '')
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState('')

  useEffect(() => {
    setName(dept.name); setDescription(dept.description || ''); setParentId(dept.parent_id || ''); setError('')
  }, [dept, open])

  // 前端先排除自身；服务端仍会做父子循环校验。
  const availableParents = departments.filter(d => d.id !== dept.id)
  const rootAllowed = canBeRootDepartment(departments, dept)

  const handleSave = async () => {
    if (name.trim().length < 2) { setError('部门名称至少 2 个字符'); return }
    if (!rootAllowed && !parentId) { setError('系统只允许一个顶级部门，请为该部门选择上级部门'); return }
    setSaving(true); setError('')
    try {
      await api.put(`/departments/${dept.id}`, {
        name: name.trim(),
        description: description || null,
        parent_id: parentId || null,
      })
      onSaved()
    } catch (err: any) {
      setError(err.response?.data?.details || err.response?.data?.error || '更新失败')
    } finally { setSaving(false) }
  }

  return (
    <Dialog open={open} onOpenChange={(v) => { if (!v) onClose() }}>
      <DialogContent className="sm:max-w-[520px] rounded-none p-0 gap-0 ring-0" style={{ border: '1px solid #E8E8E8' }} showCloseButton={false}>
        {error && <StatusMessage type="error" message={error} />}
        <div className="flex items-center justify-between px-6 py-5" style={{ borderBottom: '1px solid #E8E8E8' }}>
          <span className="font-mono text-xs font-semibold text-[#1A1A1A]">编辑部门</span>
        </div>
        <div className="flex flex-col gap-4 px-6 py-5">
          <div className="flex flex-col gap-1.5">
            <FormLabel required>部门名称</FormLabel>
            <input value={name} onChange={e => setName(e.target.value)} className="border border-[#E8E8E8] bg-[#F5F5F5] px-3 py-2 font-mono text-[10px] text-[#1A1A1A] outline-none focus:border-[#0A6B3A]" />
          </div>
          <div className="flex flex-col gap-1.5">
            <FormLabel>上级部门</FormLabel>
            <select value={parentId} onChange={e => setParentId(e.target.value)} className="border border-[#E8E8E8] bg-[#F5F5F5] px-3 py-2 font-mono text-[10px] text-[#1A1A1A] outline-none">
              {rootAllowed && <option value="">无（顶级部门）</option>}
              {availableParents.map(d => <option key={d.id} value={d.id}>{d.name}</option>)}
            </select>
            {!rootAllowed && <p className="font-mono text-[9px] text-[#999]">当前已存在顶级部门，不能再将该部门提升为顶级部门。</p>}
          </div>
          <div className="flex flex-col gap-1.5">
            <FormLabel>描述</FormLabel>
            <textarea value={description} onChange={e => setDescription(e.target.value)} rows={2} className="border border-[#E8E8E8] bg-[#F5F5F5] px-3 py-2 font-mono text-[10px] text-[#1A1A1A] outline-none resize-none" />
          </div>
        </div>
        <div className="flex justify-end gap-2 px-6 py-4" style={{ borderTop: '1px solid #E8E8E8' }}>
          <button onClick={onClose} className="border border-[#E8E8E8] bg-white px-4 py-2 font-mono text-[9px] font-semibold text-[#1A1A1A]">取消</button>
          <button onClick={handleSave} disabled={saving} className="flex items-center gap-1.5 bg-[#0A6B3A] px-4 py-2 font-mono text-[9px] font-semibold text-white disabled:opacity-50">
            {saving ? <Loader2 className="h-3 w-3 animate-spin" /> : <Save className="h-3 w-3" />}
            保存
          </button>
        </div>
      </DialogContent>
    </Dialog>
  )
}

/* ── 模型白名单编辑弹窗 ── */

function ModelWhitelistDialog({ open, deptId, deptName, currentWhitelist, onClose, onSaved }: {
  open: boolean; deptId: string; deptName: string; currentWhitelist: WhitelistModel[]; onClose: () => void; onSaved: () => void
}) {
  const [allModels, setAllModels] = useState<ModelOption[]>([])
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState('')

  useEffect(() => {
    if (!open) return
    setSelected(new Set(currentWhitelist.map(m => m.id)))
    setError('')
    api.get('/model-configs').then(res => {
      setAllModels(mapModelOptions(res.data))
    }).catch(() => setError('加载模型列表失败'))
  }, [open, currentWhitelist])

  const toggle = (id: string) => {
    setSelected((current) => toggleSelection(current, id))
  }

  const handleSave = async () => {
    setSaving(true); setError('')
    try {
      await api.put(`/departments/${deptId}/model-whitelist`, { model_config_ids: Array.from(selected) })
      onSaved()
    } catch (err: any) {
      setError(err.response?.data?.details || err.response?.data?.error || '保存失败')
    } finally { setSaving(false) }
  }

  return (
    <Dialog open={open} onOpenChange={(v) => { if (!v) onClose() }}>
      <DialogContent className="sm:max-w-[520px] rounded-none p-0 gap-0 ring-0" style={{ border: '1px solid #E8E8E8' }} showCloseButton={false}>
        {error && <StatusMessage type="error" message={error} />}
        <div className="flex items-center justify-between px-6 py-5" style={{ borderBottom: '1px solid #E8E8E8' }}>
          <span className="font-mono text-xs font-semibold text-[#1A1A1A]">编辑模型白名单 — {deptName}</span>
        </div>
        <div className="px-6 py-4">
          <p className="mb-3 font-mono text-[9px] text-[#999]">不选择任何模型 = 该部门可使用所有已启用模型</p>
          <div className="flex flex-col gap-1.5 max-h-[300px] overflow-y-auto">
            {allModels.map(m => (
              <button key={m.id} onClick={() => toggle(m.id)} className="flex items-center gap-3 px-3 py-2 text-left" style={{ border: `1px solid ${selected.has(m.id) ? '#0A6B3A' : '#E8E8E8'}`, background: selected.has(m.id) ? '#0A6B3A08' : '#fff' }}>
                <div className={`flex h-4 w-4 shrink-0 items-center justify-center border ${selected.has(m.id) ? 'border-[#0A6B3A] bg-[#0A6B3A]' : 'border-[#D9D9D9]'}`}>
                  {selected.has(m.id) && <Check className="h-2.5 w-2.5 text-white" />}
                </div>
                <div className="flex flex-col">
                  <span className="flex items-center gap-2 font-mono text-[10px] font-medium text-[#1A1A1A]">
                    {m.display_name}
                    {!m.enabled && (
                      <span className="border border-[#E8E8E8] bg-[#F5F5F5] px-1.5 py-0.5 text-[8px] text-[#8A8A8A]">
                        已禁用
                      </span>
                    )}
                  </span>
                  <span className="font-mono text-[9px] text-[#999]">{m.provider} · {m.model_id}</span>
                </div>
              </button>
            ))}
            {allModels.length === 0 && <span className="py-4 text-center font-mono text-[10px] text-[#999]">暂无可用模型</span>}
          </div>
        </div>
        <div className="flex items-center justify-between px-6 py-4" style={{ borderTop: '1px solid #E8E8E8' }}>
          <span className="font-mono text-[9px] text-[#999]">已选 {selected.size} / {allModels.length}</span>
          <div className="flex gap-2">
            <button onClick={onClose} className="border border-[#E8E8E8] bg-white px-4 py-2 font-mono text-[9px] font-semibold text-[#1A1A1A]">取消</button>
            <button onClick={handleSave} disabled={saving} className="flex items-center gap-1.5 bg-[#0A6B3A] px-4 py-2 font-mono text-[9px] font-semibold text-white disabled:opacity-50">
              {saving ? <Loader2 className="h-3 w-3 animate-spin" /> : <Save className="h-3 w-3" />}
              保存白名单
            </button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  )
}

function SkillWhitelistDialog({ open, deptId, deptName, currentWhitelist, onClose, onSaved }: {
  open: boolean; deptId: string; deptName: string; currentWhitelist: WhitelistSkill[]; onClose: () => void; onSaved: () => void
}) {
  const [allSkills, setAllSkills] = useState<SkillOption[]>([])
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState('')

  useEffect(() => {
    if (!open) return
    setSelected(new Set(currentWhitelist.map(s => s.skill_id)))
    setError('')
    api.get('/skills').then(res => {
      setAllSkills(mapApprovedSkillOptions(res.data?.skills))
    }).catch(() => setError('加载技能列表失败'))
  }, [open, currentWhitelist])

  const toggle = (id: string) => {
    setSelected((current) => toggleSelection(current, id))
  }

  const handleSave = async () => {
    setSaving(true); setError('')
    try {
      await api.put(`/departments/${deptId}/skill-whitelist`, { skill_ids: Array.from(selected) })
      onSaved()
    } catch (err: any) {
      setError(err.response?.data?.error || '保存失败')
    } finally { setSaving(false) }
  }

  return (
    <Dialog open={open} onOpenChange={(v) => { if (!v) onClose() }}>
      <DialogContent className="sm:max-w-[520px] rounded-none p-0 gap-0 ring-0" style={{ border: '1px solid #E8E8E8' }} showCloseButton={false}>
        {error && <StatusMessage type="error" message={error} />}
        <div className="flex items-center justify-between px-6 py-5" style={{ borderBottom: '1px solid #E8E8E8' }}>
          <span className="font-mono text-xs font-semibold text-[#1A1A1A]">编辑技能白名单 — {deptName}</span>
        </div>
        <div className="px-6 py-4">
          <p className="mb-3 font-mono text-[9px] text-[#999]">不选择任何技能 = 该部门可使用全部已审核技能</p>
          <div className="flex max-h-[300px] flex-col gap-1.5 overflow-y-auto">
            {allSkills.map(s => (
              <button key={s.id} onClick={() => toggle(s.id)} className="flex items-center gap-3 px-3 py-2 text-left" style={{ border: `1px solid ${selected.has(s.id) ? '#0A6B3A' : '#E8E8E8'}`, background: selected.has(s.id) ? '#0A6B3A08' : '#fff' }}>
                <div className={`flex h-4 w-4 shrink-0 items-center justify-center border ${selected.has(s.id) ? 'border-[#0A6B3A] bg-[#0A6B3A]' : 'border-[#D9D9D9]'}`}>
                  {selected.has(s.id) && <Check className="h-2.5 w-2.5 text-white" />}
                </div>
                <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{s.name}</span>
              </button>
            ))}
            {allSkills.length === 0 && <span className="py-4 text-center font-mono text-[10px] text-[#999]">暂无可用技能</span>}
          </div>
        </div>
        <div className="flex items-center justify-between px-6 py-4" style={{ borderTop: '1px solid #E8E8E8' }}>
          <span className="font-mono text-[9px] text-[#999]">已选 {selected.size} / {allSkills.length}</span>
          <div className="flex gap-2">
            <button onClick={onClose} className="border border-[#E8E8E8] bg-white px-4 py-2 font-mono text-[9px] font-semibold text-[#1A1A1A]">取消</button>
            <button onClick={handleSave} disabled={saving} className="flex items-center gap-1.5 bg-[#0A6B3A] px-4 py-2 font-mono text-[9px] font-semibold text-white disabled:opacity-50">
              {saving ? <Loader2 className="h-3 w-3 animate-spin" /> : <Save className="h-3 w-3" />}
              保存白名单
            </button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  )
}

/* ── 配额卡片（接入 API） ── */
/* 根部门（parent_id 为 null）展示子部门限额累加值 + 自定义限额切换（验收标准15） */
/* 普通部门展示直接配额设置 */

function QuotaCard({ dept, onSaved }: { dept: DepartmentDetail; onSaved: () => void }) {
  const isRoot = !dept.parent_id
  return isRoot
    ? <RootQuotaCard dept={dept} onSaved={onSaved} />
    : <DeptQuotaCard dept={dept} onSaved={onSaved} />
}

function RootQuotaCard({ dept, onSaved }: { dept: DepartmentDetail; onSaved: () => void }) {
  const [summary, setSummary] = useState<RootDepartmentQuotaSummary | null>(null)
  const [isCustom, setIsCustom] = useState(false)
  const [customLimit, setCustomLimit] = useState('')
  const [monthlyBudget, setMonthlyBudget] = useState('')
  const [saving, setSaving] = useState(false)
  const [msg, setMsg] = useState<{ type: 'success' | 'error'; text: string } | null>(null)

  useEffect(() => {
    setMsg(null)
    api.get(`/departments/${dept.id}/quota-summary`).then(res => {
      setSummary(res.data)
      setIsCustom(res.data.is_custom)
      setCustomLimit(res.data.custom_limit_cents != null ? String(res.data.custom_limit_cents / 100) : '')
      setMonthlyBudget(res.data.monthly_budget_cents != null ? String(res.data.monthly_budget_cents / 100) : '')
    }).catch(() => setSummary(null))
  }, [dept.id])

  const handleSave = async () => {
    setSaving(true); setMsg(null)
    try {
      await api.put('/quota/config', {
        org_daily_limit_cents: isCustom && customLimit ? Math.round(Number(customLimit) * 100) : null,
        monthly_budget_cents: monthlyBudget ? Math.round(Number(monthlyBudget) * 100) : null,
      })
      setMsg({ type: 'success', text: '公司级配额已保存' })
      onSaved()
    } catch (err: any) {
      setMsg({ type: 'error', text: err.response?.data?.error || '保存失败' })
    } finally { setSaving(false) }
  }

  const displayLimit = rootQuotaLimitLabel(summary, isCustom, customLimit)

  const budgetPct = summary && summary.monthly_budget_cents
    ? Math.min(Math.round(summary.month_total_used_cents / summary.monthly_budget_cents * 100), 100)
    : null

  return (
    <div className="flex flex-1 flex-col gap-3 bg-white p-5" style={{ border: '1px solid #E8E8E8' }}>
      <div className="flex items-center gap-2.5 pb-3" style={{ borderBottom: '1px solid #E8E8E8' }}>
        <Coins className="h-3.5 w-3.5 text-[#0A6B3A]" />
        <span className="font-mono text-[11px] font-semibold tracking-[0.5px] text-[#1A1A1A]">公司级配额设置</span>
      </div>
      {msg && <StatusMessage type={msg.type} message={msg.text} />}
      {summary && (
        <div className="flex gap-2">
          <div className="flex flex-1 items-center justify-between bg-[#F5F5F5] px-3 py-2" style={{ border: '1px solid #E8E8E8' }}>
            <span className="font-mono text-[9px] text-[#999]">今日消耗</span>
            <span className="font-mono text-[10px] font-semibold text-[#1A1A1A]">{formatCents(summary.today_total_used_cents)}</span>
          </div>
          <div className="flex flex-1 items-center justify-between bg-[#F5F5F5] px-3 py-2" style={{ border: '1px solid #E8E8E8' }}>
            <span className="font-mono text-[9px] text-[#999]">本月消耗</span>
            <span className="font-mono text-[10px] font-semibold text-[#1A1A1A]">{formatCents(summary.month_total_used_cents)}</span>
          </div>
        </div>
      )}
      <div className="flex items-center justify-between">
        <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">每日限额</span>
        <span className="font-mono text-[10px] font-medium text-[#0A6B3A]">{displayLimit}</span>
      </div>
      <div className="flex items-center justify-between">
        <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">自定义限额</span>
        <button onClick={() => setIsCustom(!isCustom)} className={`h-5 w-9 rounded-full transition-colors ${isCustom ? 'bg-[#0A6B3A]' : 'bg-[#D9D9D9]'}`}>
          <div className={`h-4 w-4 rounded-full bg-white shadow transition-transform ${isCustom ? 'translate-x-4' : 'translate-x-0.5'}`} />
        </button>
      </div>
      {!isCustom && summary && (
        <p className="font-mono text-[9px] text-[#999]">默认使用子部门限额累加值 ({formatCents(summary.children_limit_sum_cents)})。开启自定义后可手动设置更严格的总量控制。</p>
      )}
      {isCustom && (
        <div className="flex items-center justify-between">
          <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">日限额</span>
          <div className="flex items-center gap-2 bg-[#F5F5F5] px-2.5 py-1.5" style={{ border: '1px solid #E8E8E8' }}>
            <input type="number" value={customLimit} onChange={e => setCustomLimit(e.target.value)} min={0} step="0.01" className="w-24 bg-transparent font-mono text-[10px] text-[#1A1A1A] outline-none" />
            <span className="font-mono text-[9px] text-[#999]">元</span>
          </div>
        </div>
      )}
      <div className="flex items-center justify-between pt-2" style={{ borderTop: '1px solid #E8E8E8' }}>
        <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">月度预算</span>
        <div className="flex items-center gap-2 bg-[#F5F5F5] px-2.5 py-1.5" style={{ border: '1px solid #E8E8E8' }}>
          <input type="number" value={monthlyBudget} onChange={e => setMonthlyBudget(e.target.value)} min={0} step="0.01" className="w-24 bg-transparent font-mono text-[10px] text-[#1A1A1A] outline-none" placeholder="不填则不限" />
          <span className="font-mono text-[9px] text-[#999]">元</span>
        </div>
      </div>
      {budgetPct != null && (
        <div className="flex flex-col gap-1">
          <div className="h-2 w-full overflow-hidden bg-[#F0F0F0]">
            <div className="h-full" style={{ width: `${budgetPct}%`, background: budgetPct > 90 ? '#CF1322' : '#0A6B3A' }} />
          </div>
          <span className="font-mono text-[9px]" style={{ color: budgetPct > 90 ? '#CF1322' : '#999' }}>预算已用 {budgetPct}%</span>
        </div>
      )}
      <div className="flex justify-end gap-2 pt-2" style={{ borderTop: '1px solid #E8E8E8' }}>
        <button onClick={handleSave} disabled={saving} className="flex items-center gap-1.5 bg-[#0A6B3A] px-4 py-2 font-mono text-[9px] font-semibold text-white disabled:opacity-50">
          {saving ? <Loader2 className="h-2.5 w-2.5 animate-spin" /> : <Save className="h-2.5 w-2.5" />}
          保存配额
        </button>
      </div>
    </div>
  )
}

function DeptQuotaCard({ dept, onSaved }: { dept: DepartmentDetail; onSaved: () => void }) {
  const [limitEnabled, setLimitEnabled] = useState(false)
  const [limitCents, setLimitCents] = useState('')
  const [saving, setSaving] = useState(false)
  const [msg, setMsg] = useState<{ type: 'success' | 'error'; text: string } | null>(null)

  useEffect(() => {
    setMsg(null)
    // 从 quota_configs 加载部门限额
    api.get(`/departments/${dept.id}/quota-summary`).then(res => {
      const limit = res.data.custom_limit_cents
      setLimitEnabled(limit != null)
      setLimitCents(limit != null ? String(limit / 100) : '')
    }).catch(() => {
      setLimitEnabled(dept.token_quota_enabled)
      setLimitCents('')
    })
  }, [dept])

  const handleSave = async () => {
    setSaving(true); setMsg(null)
    try {
      // 写入 quota_configs 表
      const body: Record<string, unknown> = { scope: 'department', scope_id: dept.id }
      if (limitEnabled && limitCents) {
        body.daily_limit_cents = Number(limitCents)
      } else {
        body.daily_limit_cents = null
      }
      await api.put(`/departments/${dept.id}`, {
        token_quota_enabled: limitEnabled,
        token_quota_per_day: limitEnabled && limitCents ? Math.round(Number(limitCents) * 100) : null,
      })
      setMsg({ type: 'success', text: '配额已保存' })
      onSaved()
    } catch (err: any) {
      setMsg({ type: 'error', text: err.response?.data?.error || '保存失败' })
    } finally { setSaving(false) }
  }

  const handleReset = () => {
    setMsg(null)
    api.get(`/departments/${dept.id}/quota-summary`).then(res => {
      const limit = res.data.custom_limit_cents
      setLimitEnabled(limit != null)
      setLimitCents(limit != null ? String(limit / 100) : '')
    }).catch(() => {})
  }

  return (
    <div className="flex flex-1 flex-col gap-3 bg-white p-5" style={{ border: '1px solid #E8E8E8' }}>
      <div className="flex items-center gap-2.5 pb-3" style={{ borderBottom: '1px solid #E8E8E8' }}>
        <Coins className="h-3.5 w-3.5 text-[#0A6B3A]" />
        <span className="font-mono text-[11px] font-semibold tracking-[0.5px] text-[#1A1A1A]">费用限额设置</span>
      </div>
      {msg && <StatusMessage type={msg.type} message={msg.text} />}
      <div className="flex items-center justify-between">
        <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">启用限额</span>
        <button onClick={() => setLimitEnabled(!limitEnabled)} className={`h-5 w-9 rounded-full transition-colors ${limitEnabled ? 'bg-[#0A6B3A]' : 'bg-[#D9D9D9]'}`}>
          <div className={`h-4 w-4 rounded-full bg-white shadow transition-transform ${limitEnabled ? 'translate-x-4' : 'translate-x-0.5'}`} />
        </button>
      </div>
      {limitEnabled && (
        <div className="flex items-center justify-between">
          <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">每日限额</span>
          <div className="flex items-center gap-2 bg-[#F5F5F5] px-2.5 py-1.5" style={{ border: '1px solid #E8E8E8' }}>
            <input type="number" value={limitCents} onChange={e => setLimitCents(e.target.value)} min={0} step="0.01" className="w-24 bg-transparent font-mono text-[10px] text-[#1A1A1A] outline-none" />
            <span className="font-mono text-[9px] text-[#999]">元</span>
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

function AccessCard({
  modelWhitelist,
  skillWhitelist,
  onEditModel,
  onEditSkill,
}: {
  modelWhitelist: WhitelistModel[]
  skillWhitelist: WhitelistSkill[]
  onEditModel: () => void
  onEditSkill: () => void
}) {
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
            {modelWhitelist.length > 0 ? `${modelWhitelist.length} 个模型` : '全部可用'}
          </span>
          <button onClick={onEditModel} className="flex items-center gap-1.5 border border-[#0A6B3A] bg-white px-2 py-0.5 font-mono text-[8px] font-semibold text-[#0A6B3A]">
            <Pencil className="h-2.5 w-2.5" />
            编辑
          </button>
        </div>
      </div>
      {modelWhitelist.length > 0 && (
        <div className="flex flex-wrap gap-1.5">
          {modelWhitelist.map(m => (
            <span key={m.id} className="bg-[#F5F5F5] px-2 py-1 font-mono text-[9px] text-[#1A1A1A]" style={{ border: '1px solid #E8E8E8' }}>
              {m.display_name}
            </span>
          ))}
        </div>
      )}

      <div className="mt-1 flex items-center justify-between" style={{ borderTop: '1px dashed #E8E8E8', paddingTop: '10px' }}>
        <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">可用技能白名单</span>
        <div className="flex items-center gap-2">
          <span className="font-mono text-[10px] font-medium text-[#0A6B3A]">
            {skillWhitelist.length > 0 ? `${skillWhitelist.length} 个技能` : '全部可用'}
          </span>
          <button onClick={onEditSkill} className="flex items-center gap-1.5 border border-[#0A6B3A] bg-white px-2 py-0.5 font-mono text-[8px] font-semibold text-[#0A6B3A]">
            <Pencil className="h-2.5 w-2.5" />
            编辑
          </button>
        </div>
      </div>
      {skillWhitelist.length > 0 && (
        <div className="flex flex-wrap gap-1.5">
          {skillWhitelist.map(s => (
            <span key={s.skill_id} className="bg-[#F5F5F5] px-2 py-1 font-mono text-[9px] text-[#1A1A1A]" style={{ border: '1px solid #E8E8E8' }}>
              {s.name}
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
  { key: 'email', label: '邮箱', width: 'w-[200px]' },
  { key: 'created_at', label: '加入时间', width: 'w-[160px]' },
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
            <span className="w-[200px] font-mono text-[10px] font-medium text-[#1A1A1A] truncate">{m.email || '-'}</span>
            <span className="w-[160px] font-mono text-[10px] font-medium text-[#999]">{new Date(m.created_at).toLocaleDateString('zh-CN')}</span>
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
  const [editOpen, setEditOpen] = useState(false)
  const [whitelistOpen, setWhitelistOpen] = useState(false)
  const [skillWhitelistOpen, setSkillWhitelistOpen] = useState(false)
  const [searchQuery, setSearchQuery] = useState('')
  const [quotaFilter, setQuotaFilter] = useState('')

  // 选中部门的详情数据
  const [detail, setDetail] = useState<DepartmentDetail | null>(null)
  const [members, setMembers] = useState<DeptMember[]>([])
  const [whitelist, setWhitelist] = useState<WhitelistModel[]>([])
  const [skillWhitelist, setSkillWhitelist] = useState<WhitelistSkill[]>([])
  const [quotaSummary, setQuotaSummary] = useState<DepartmentQuotaSummary | null>(null)
  const [detailLoading, setDetailLoading] = useState(false)

  const loadDepartments = useCallback(async () => {
    setLoading(true)
    try {
      const params: Record<string, string> = {}
      if (searchQuery.trim()) params.search = searchQuery.trim()
      if (quotaFilter) params.quota_status = quotaFilter
      const res = await api.get('/departments', { params })
      setDepartments(res.data.departments || [])
    } catch (err) {
      console.error('加载部门列表失败', err)
    } finally { setLoading(false) }
  }, [searchQuery, quotaFilter])

  const loadDetail = useCallback(async (id: string) => {
    setDetailLoading(true)
    try {
      const [dRes, mRes, wRes, sRes, qRes] = await Promise.all([
        api.get(`/departments/${id}`),
        api.get(`/departments/${id}/members`),
        api.get(`/departments/${id}/model-whitelist`),
        api.get(`/departments/${id}/skill-whitelist`),
        api.get(`/departments/${id}/quota-summary`).catch(() => ({ data: null })),
      ])
      setDetail(dRes.data)
      setMembers(mRes.data.members || [])
      setWhitelist(wRes.data.models || [])
      setSkillWhitelist(sRes.data.skills || [])
      setQuotaSummary(qRes.data)
    } catch (err) {
      console.error('加载部门详情失败', err)
    } finally { setDetailLoading(false) }
  }, [])

  useEffect(() => { loadDepartments() }, [loadDepartments])

  // 选中部门变化时加载详情
  useEffect(() => {
    if (selectedId) loadDetail(selectedId)
    else { setDetail(null); setMembers([]); setWhitelist([]); setSkillWhitelist([]); setQuotaSummary(null) }
  }, [selectedId, loadDetail])

  // 加载完成后默认选中第一个
  useEffect(() => {
    if (departments.length > 0 && !selectedId) setSelectedId(departments[0].id)
  }, [departments, selectedId])

  const tree = useMemo(() => buildTree(departments), [departments])

  const handleCreated = () => { setCreateOpen(false); loadDepartments() }
  const handleDeleted = () => { setSelectedId(''); loadDepartments() }
  const handleQuotaSaved = () => { loadDepartments(); if (selectedId) loadDetail(selectedId) }
  const handleEditSaved = () => { setEditOpen(false); loadDepartments(); if (selectedId) loadDetail(selectedId) }
  const handleWhitelistSaved = () => { setWhitelistOpen(false); if (selectedId) loadDetail(selectedId) }
  const handleSkillWhitelistSaved = () => { setSkillWhitelistOpen(false); if (selectedId) loadDetail(selectedId) }

  return (
    <div className="flex flex-col gap-6">
      {/* 标题 + 新建按钮 */}
      <div className="flex items-end justify-between">
        <div className="flex flex-col gap-2">
          <h1 className="text-[22px] font-bold tracking-tight text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px' }}>
            部门管理
          </h1>
          <p className="font-mono text-xs text-[#999]">组织架构、费用配额、知识库权限与模型白名单管理</p>
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
          {/* 搜索 + 筛选 */}
          <div className="flex flex-col gap-2">
            <div className="flex items-center gap-1.5 border border-[#E8E8E8] bg-[#F5F5F5] px-2.5 py-1.5">
              <Search className="h-3 w-3 text-[#999]" />
              <input
                value={searchQuery}
                onChange={e => setSearchQuery(e.target.value)}
                placeholder="搜索部门..."
                className="flex-1 bg-transparent font-mono text-[10px] text-[#1A1A1A] outline-none placeholder:text-[#CCCCCC]"
              />
            </div>
            <select
              value={quotaFilter}
              onChange={e => setQuotaFilter(e.target.value)}
              className="border border-[#E8E8E8] bg-[#F5F5F5] px-2.5 py-1.5 font-mono text-[10px] text-[#1A1A1A] outline-none"
            >
              <option value="">全部状态</option>
              <option value="enabled">已启用限额</option>
              <option value="disabled">未启用限额</option>
            </select>
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
              {/* 部门名称 + 编辑按钮 */}
              <div className="flex items-center justify-between">
                <div>
                  <span className="text-lg font-bold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>{detail.name}</span>
                  {detail.description && <p className="mt-1 font-mono text-[10px] text-[#999]">{detail.description}</p>}
                </div>
                <button onClick={() => setEditOpen(true)} className="flex items-center gap-1.5 border border-[#E8E8E8] px-3 py-1.5 font-mono text-[9px] font-semibold text-[#1A1A1A]">
                  <Pencil className="h-3 w-3" />
                  编辑部门
                </button>
              </div>

              {/* 统计卡片 */}
              <div className="flex gap-3">
                {[
                  { label: '成员数', value: String(detail.member_count), color: '#1A1A1A', border: '#E8E8E8' },
                  { label: '每日消耗', value: quotaSummary ? formatCents(quotaSummary.today_total_used_cents) : '—', color: '#0A6B3A', border: '#0A6B3A40' },
                  { label: '每日配额', value: departmentDailyQuotaLabel(detail, quotaSummary), color: '#1A1A1A', border: '#E8E8E8' },
                  { label: '模型白名单', value: whitelist.length > 0 ? `${whitelist.length} 个` : '全部', color: '#1A1A1A', border: '#E8E8E8' },
                  { label: '技能白名单', value: skillWhitelist.length > 0 ? `${skillWhitelist.length} 个` : '全部', color: '#1A1A1A', border: '#E8E8E8' },
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
                <AccessCard
                  modelWhitelist={whitelist}
                  skillWhitelist={skillWhitelist}
                  onEditModel={() => setWhitelistOpen(true)}
                  onEditSkill={() => setSkillWhitelistOpen(true)}
                />
              </div>

              {/* 成员表格 */}
              <MemberTable dept={detail} members={members} onDelete={handleDeleted} />
            </>
          ) : null}
        </div>
      </div>

      <CreateDeptDialog open={createOpen} departments={departments} onClose={() => setCreateOpen(false)} onCreated={handleCreated} />
      {detail && <EditDeptDialog open={editOpen} dept={detail} departments={departments} onClose={() => setEditOpen(false)} onSaved={handleEditSaved} />}
      {detail && <ModelWhitelistDialog open={whitelistOpen} deptId={detail.id} deptName={detail.name} currentWhitelist={whitelist} onClose={() => setWhitelistOpen(false)} onSaved={handleWhitelistSaved} />}
      {detail && <SkillWhitelistDialog open={skillWhitelistOpen} deptId={detail.id} deptName={detail.name} currentWhitelist={skillWhitelist} onClose={() => setSkillWhitelistOpen(false)} onSaved={handleSkillWhitelistSaved} />}
    </div>
  )
}
