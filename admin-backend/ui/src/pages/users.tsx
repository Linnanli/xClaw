import { Search, Upload, RefreshCw, Plus, Pencil, Trash2, Users, Shield, Lock, Loader2, Check, UserPlus } from 'lucide-react'
import { useState, useEffect, useCallback } from 'react'
import { TablePagination } from '@/components/ui/table-pagination'
import { StatusTag } from '@/components/ui/status-tag'
import { Dialog, DialogContent } from '@/components/ui/dialog'
import { api } from '@/lib/api'

/* ── 类型定义 ── */

interface User {
  id: string
  username: string
  email: string
  roles?: Array<{ id: string; name: string }>
  department?: { id: string; name: string } | null
  created_at: string
  updated_at: string
}

interface Role {
  id: string
  name: string
  description?: string
  permission_count: number
  user_count: number
  created_at: string
}

interface Permission {
  id: string
  name: string
  description: string
  resource: string
  action: string
}

import { FormLabel, StatusMessage } from '@/components/ui/form-helpers'

/* ── 角色颜色映射 ── */

const ROLE_COLORS: Record<string, string> = {
  '超级管理员': '#0A6B3A',
  '管理员': '#1677FF',
  '审计员': '#D48700',
}

function getRoleColor(name: string): string {
  return ROLE_COLORS[name] ?? '#999999'
}

/* ── 新建用户弹窗 ── */

interface CreateUserDialogProps {
  open: boolean
  onClose: () => void
  onCreated: () => void
}

function CreateUserDialog({ open, onClose, onCreated }: CreateUserDialogProps) {
  const [username, setUsername] = useState('')
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [confirmPassword, setConfirmPassword] = useState('')
  const [saving, setSaving] = useState(false)
  const [message, setMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null)

  useEffect(() => {
    if (!open) return
    setUsername('')
    setEmail('')
    setPassword('')
    setConfirmPassword('')
    setMessage(null)
    setSaving(false)
  }, [open])

  async function handleSubmit() {
    if (!username.trim() || !email.trim() || !password.trim() || !confirmPassword.trim()) {
      setMessage({ type: 'error', text: '请填写所有必填字段' })
      return
    }
    if (password !== confirmPassword) {
      setMessage({ type: 'error', text: '两次输入的密码不一致' })
      return
    }
    setSaving(true)
    setMessage(null)
    try {
      await api.post('/auth/register', { username: username.trim(), email: email.trim(), password })
      setMessage({ type: 'success', text: '用户创建成功' })
      setTimeout(() => { onCreated(); onClose() }, 500)
    } catch (err: any) {
      const msg = err?.response?.data?.error || '创建用户失败'
      setMessage({ type: 'error', text: msg })
      console.error('创建用户失败:', err)
    } finally {
      setSaving(false)
    }
  }

  const inputClass = 'w-full bg-[#F5F5F5] px-3.5 py-2.5 font-mono text-[10px] font-medium text-[#1A1A1A] outline-none placeholder:text-[#CCCCCC]'
  const inputStyle = { border: '1px solid #E8E8E8' }

  return (
    <Dialog open={open} onOpenChange={(v) => { if (!v) onClose() }}>
      <DialogContent className="sm:max-w-[480px] rounded-none p-0 gap-0 ring-0" style={{ border: '1px solid #E8E8E8' }} showCloseButton={false}>
        {/* 头部 */}
        <div className="flex items-center justify-between px-6 py-5" style={{ borderBottom: '1px solid #E8E8E8' }}>
          <div className="flex items-center gap-2.5">
            <UserPlus className="h-4 w-4 text-[#0A6B3A]" />
            <span className="text-base font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>新建用户</span>
          </div>
          <button onClick={onClose} className="font-mono text-sm font-semibold text-[#999999] hover:text-[#1A1A1A]">✕</button>
        </div>

        {message && <StatusMessage type={message.type} message={message.text} />}

        {/* 表单 */}
        <div className="flex flex-col gap-4 px-6 py-6">
          <div className="flex flex-col gap-2">
            <FormLabel required>用户名</FormLabel>
            <input value={username} onChange={(e) => setUsername(e.target.value)} placeholder="请输入用户名" className={inputClass} style={inputStyle} />
          </div>
          <div className="flex flex-col gap-2">
            <FormLabel required>邮箱</FormLabel>
            <input type="email" value={email} onChange={(e) => setEmail(e.target.value)} placeholder="请输入邮箱" className={inputClass} style={inputStyle} />
          </div>
          <div className="flex flex-col gap-2">
            <FormLabel required>密码</FormLabel>
            <input type="password" value={password} onChange={(e) => setPassword(e.target.value)} placeholder="请输入密码" className={inputClass} style={inputStyle} />
          </div>
          <div className="flex flex-col gap-2">
            <FormLabel required>确认密码</FormLabel>
            <input type="password" value={confirmPassword} onChange={(e) => setConfirmPassword(e.target.value)} placeholder="请再次输入密码" className={inputClass} style={inputStyle} />
          </div>
        </div>

        {/* 底部按钮 */}
        <div className="flex items-center justify-end gap-3 px-6 py-4" style={{ borderTop: '1px solid #E8E8E8' }}>
          <button onClick={onClose} className="border border-[#E8E8E8] bg-white px-5 py-2.5 font-mono text-[10px] font-semibold text-[#1A1A1A]">取消</button>
          <button onClick={handleSubmit} disabled={saving} className="flex items-center gap-2 bg-[#0A6B3A] px-5 py-2.5 font-mono text-[10px] font-semibold text-white disabled:opacity-50">
            {saving ? <Loader2 className="h-3 w-3 animate-spin" /> : <Check className="h-3 w-3" />}
            创建
          </button>
        </div>
      </DialogContent>
    </Dialog>
  )
}

/* ── Tab 定义 ── */

const tabs = ['用户列表', '角色管理', '权限管理'] as const
type TabKey = (typeof tabs)[number]

/* ── 主组件 ── */

export default function UsersPage() {
  const [activeTab, setActiveTab] = useState<TabKey>('用户列表')
  const [createDialogOpen, setCreateDialogOpen] = useState(false)

  // 用户列表状态
  const [users, setUsers] = useState<User[]>([])
  const [usersLoading, setUsersLoading] = useState(false)
  const [userSearch, setUserSearch] = useState('')
  const [userPage, setUserPage] = useState(1)

  // 角色管理状态
  const [roles, setRoles] = useState<Role[]>([])
  const [rolesLoading, setRolesLoading] = useState(false)

  // 权限管理状态
  const [permissions, setPermissions] = useState<Record<string, Permission[]>>({})
  const [permissionsLoading, setPermissionsLoading] = useState(false)

  /* ── 数据加载 ── */

  const loadUsers = useCallback(async () => {
    setUsersLoading(true)
    try {
      const resp = await api.get('/users')
      setUsers(resp.data.users ?? [])
    } catch (err) {
      console.error('加载用户列表失败:', err)
      setUsers([])
    } finally {
      setUsersLoading(false)
    }
  }, [])

  const loadRoles = useCallback(async () => {
    setRolesLoading(true)
    try {
      const resp = await api.get('/roles')
      setRoles(resp.data.roles ?? [])
    } catch (err) {
      console.error('加载角色列表失败:', err)
      setRoles([])
    } finally {
      setRolesLoading(false)
    }
  }, [])

  const loadPermissions = useCallback(async () => {
    setPermissionsLoading(true)
    try {
      const resp = await api.get('/permissions')
      setPermissions(resp.data.permissions ?? {})
    } catch (err) {
      console.error('加载权限列表失败:', err)
      setPermissions({})
    } finally {
      setPermissionsLoading(false)
    }
  }, [])

  // Tab 激活时加载对应数据
  useEffect(() => {
    if (activeTab === '用户列表') loadUsers()
    else if (activeTab === '角色管理') loadRoles()
    else if (activeTab === '权限管理') loadPermissions()
  }, [activeTab, loadUsers, loadRoles, loadPermissions])

  /* ── 用户操作 ── */

  async function handleDeleteUser(user: User) {
    if (!confirm(`确定要删除用户「${user.username}」吗？`)) return
    try {
      await api.delete(`/users/${user.id}`)
      loadUsers()
    } catch (err) {
      console.error('删除用户失败:', err)
    }
  }

  async function handleDeleteRole(role: Role) {
    if (!confirm(`确定要删除角色「${role.name}」吗？`)) return
    try {
      await api.delete(`/roles/${role.id}`)
      loadRoles()
    } catch (err) {
      console.error('删除角色失败:', err)
    }
  }

  function handleTabChange(tab: TabKey) {
    setActiveTab(tab)
    setUserPage(1)
  }

  /* ── 用户搜索过滤 ── */

  const filteredUsers = users.filter((u) => {
    if (!userSearch) return true
    const kw = userSearch.toLowerCase()
    return u.username.toLowerCase().includes(kw) || u.email.toLowerCase().includes(kw)
  })

  const PAGE_SIZE = 10
  const pagedUsers = filteredUsers.slice((userPage - 1) * PAGE_SIZE, userPage * PAGE_SIZE)

  /* ── 资源名映射 ── */

  const resourceNames: Record<string, string> = {
    users: '用户管理',
    roles: '角色管理',
    permissions: '权限管理',
    dlp: 'DLP 管理',
    audit: '审计日志',
    clients: '客户端管理',
    departments: '部门管理',
    model_configs: '模型配置',
    settings: '系统设置',
  }

  return (
    <div className="flex flex-col gap-6">

      {/* 标题区 */}
      <div className="flex items-end justify-between">
        <div className="flex flex-col gap-2">
          <h1 className="text-[22px] font-bold tracking-tight text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px' }}>
            用户管理
          </h1>
          <p className="font-mono text-xs text-[#999999]">用户账户、角色与权限管理</p>
        </div>
        <div className="flex gap-2">
          <button className="flex items-center gap-2 border border-[#E8E8E8] bg-white px-4 py-2.5 font-mono text-[9px] font-semibold text-[#1A1A1A]">
            <Upload className="h-3 w-3 text-[#6a6a6a]" />
            批量导入
          </button>
          <button className="flex items-center gap-2 border border-[#E8E8E8] bg-white px-4 py-2.5 font-mono text-[9px] font-semibold text-[#1A1A1A]">
            <RefreshCw className="h-3 w-3 text-[#6a6a6a]" />
            LDAP 同步
          </button>
          <button onClick={() => setCreateDialogOpen(true)} className="flex items-center gap-2 bg-[#0A6B3A] px-4 py-2.5 font-mono text-[9px] font-semibold text-white">
            <Plus className="h-3 w-3" />
            新建用户
          </button>
        </div>
      </div>

      {/* Tab 栏 */}
      <div className="flex gap-0" style={{ borderBottom: '1px solid #E8E8E8' }}>
        {tabs.map((tab) => (
          <button
            key={tab}
            onClick={() => handleTabChange(tab)}
            className="px-5 py-3 font-mono text-[10px] font-semibold"
            style={{
              color: activeTab === tab ? '#0A6B3A' : '#999999',
              borderBottom: activeTab === tab ? '2px solid #0A6B3A' : '2px solid transparent',
            }}
          >
            {tab}
          </button>
        ))}
      </div>

      {/* ── 用户列表 Tab ── */}
      {activeTab === '用户列表' && (
        <>
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <div className="flex w-[260px] items-center gap-2.5 border border-[#E8E8E8] bg-white px-3.5 py-2">
                <Search className="h-3 w-3 text-[#6a6a6a]" />
                <input
                  value={userSearch}
                  onChange={(e) => { setUserSearch(e.target.value); setUserPage(1) }}
                  placeholder="搜索用户名、邮箱..."
                  className="w-full bg-transparent font-mono text-[10px] font-medium text-[#1A1A1A] outline-none placeholder:text-[#CCCCCC]"
                />
              </div>
            </div>
            <span className="font-mono text-[10px] font-medium text-[#999999]">共 {filteredUsers.length} 个用户</span>
          </div>

          <div className="bg-white" style={{ border: '1px solid #E8E8E8' }}>
            {/* 表头 */}
            <div className="flex items-center px-4 py-2.5" style={{ borderBottom: '1px solid #E8E8E8', backgroundColor: '#FAFAFA' }}>
              {['用户名', '邮箱', '角色', '部门', '创建时间', '操作'].map((h) => (
                <span
                  key={h}
                  className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]"
                  style={{
                    width: h === '操作' ? 80 : undefined,
                    flex: h === '操作' ? undefined : '1 1 0%',
                    minWidth: 0,
                    flexShrink: h === '操作' ? 0 : undefined,
                  }}
                >
                  {h}
                </span>
              ))}
            </div>

            {/* 数据行 */}
            {usersLoading ? (
              <div className="flex flex-col items-center justify-center gap-3 py-16">
                <Loader2 className="h-6 w-6 animate-spin text-[#CCCCCC]" />
                <span className="font-mono text-[10px] font-medium text-[#999999]">加载中...</span>
              </div>
            ) : pagedUsers.length === 0 ? (
              <div className="flex flex-col items-center justify-center gap-3 py-16">
                <Users className="h-10 w-10 text-[#CCCCCC]" />
                <span className="font-mono text-[10px] font-medium text-[#999999]">
                  {userSearch ? '没有匹配的用户' : '暂无用户数据'}
                </span>
              </div>
            ) : (
              pagedUsers.map((u, i) => (
                <div key={u.id} className="flex items-center px-4 py-3" style={{ borderBottom: i < pagedUsers.length - 1 ? '1px solid #E8E8E8' : 'none' }}>
                  <span className="flex-1 min-w-0 font-mono text-[10px] font-medium text-[#1A1A1A]">{u.username}</span>
                  <span className="flex-1 min-w-0 font-mono text-[10px] font-medium text-[#999999]">{u.email}</span>
                  <div className="flex-1 min-w-0 flex flex-wrap gap-1">
                    {u.roles && u.roles.length > 0 ? (
                      u.roles.map((r) => <StatusTag key={r.id} label={r.name} color={getRoleColor(r.name)} />)
                    ) : (
                      <span className="font-mono text-[10px] text-[#CCCCCC]">未分配</span>
                    )}
                  </div>
                  <span className="flex-1 min-w-0 font-mono text-[10px] font-medium text-[#1A1A1A]">
                    {u.department?.name ?? <span className="text-[#CCCCCC]">—</span>}
                  </span>
                  <span className="flex-1 min-w-0 font-mono text-[10px] font-medium text-[#999999]">
                    {new Date(u.created_at).toLocaleString('zh-CN')}
                  </span>
                  <div className="flex items-center gap-3" style={{ width: 80, flexShrink: 0 }}>
                    <button className="text-[#6a6a6a] hover:text-[#0A6B3A]" title="编辑"><Pencil className="h-3.5 w-3.5" /></button>
                    <button onClick={() => handleDeleteUser(u)} className="text-[#6a6a6a] hover:text-[#CF1322]" title="删除"><Trash2 className="h-3.5 w-3.5" /></button>
                  </div>
                </div>
              ))
            )}

            <TablePagination current={userPage} total={filteredUsers.length} pageSize={PAGE_SIZE} onChange={setUserPage} />
          </div>
        </>
      )}

      {/* ── 角色管理 Tab ── */}
      {activeTab === '角色管理' && (
        <>
          {rolesLoading ? (
            <div className="flex flex-col items-center justify-center gap-3 py-16">
              <Loader2 className="h-6 w-6 animate-spin text-[#CCCCCC]" />
              <span className="font-mono text-[10px] font-medium text-[#999999]">加载中...</span>
            </div>
          ) : roles.length === 0 ? (
            <div className="flex flex-col items-center justify-center gap-3 py-16">
              <Shield className="h-10 w-10 text-[#CCCCCC]" />
              <span className="font-mono text-[10px] font-medium text-[#999999]">暂无角色数据</span>
            </div>
          ) : (
            <div className="grid grid-cols-2 gap-4">
              {roles.map((r) => {
                const color = getRoleColor(r.name)
                return (
                  <div key={r.id} className="flex flex-col gap-3 bg-white p-5" style={{ border: '1px solid #E8E8E8' }}>
                    <div className="flex items-center justify-between">
                      <span className="text-sm font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>{r.name}</span>
                      <StatusTag label={`${r.user_count} 人`} color={color} />
                    </div>
                    <span className="font-mono text-[10px] font-medium text-[#999999]">{r.description || '暂无描述'}</span>
                    <div className="flex items-center justify-between">
                      <span className="font-mono text-[10px] font-medium text-[#999999]">{r.permission_count} 项权限</span>
                      <div className="flex items-center gap-3">
                        <button className="font-mono text-[10px] font-semibold text-[#0A6B3A]">编辑</button>
                        <button onClick={() => handleDeleteRole(r)} className="font-mono text-[10px] font-semibold text-[#CF1322]">删除</button>
                      </div>
                    </div>
                  </div>
                )
              })}
            </div>
          )}
        </>
      )}

      {/* ── 权限管理 Tab ── */}
      {activeTab === '权限管理' && (
        <>
          {permissionsLoading ? (
            <div className="flex flex-col items-center justify-center gap-3 py-16">
              <Loader2 className="h-6 w-6 animate-spin text-[#CCCCCC]" />
              <span className="font-mono text-[10px] font-medium text-[#999999]">加载中...</span>
            </div>
          ) : Object.keys(permissions).length === 0 ? (
            <div className="flex flex-col items-center justify-center gap-3 py-16">
              <Lock className="h-10 w-10 text-[#CCCCCC]" />
              <span className="font-mono text-[10px] font-medium text-[#999999]">暂无权限数据</span>
            </div>
          ) : (
            <div className="flex flex-col gap-4">
              {Object.entries(permissions).map(([resource, perms]) => (
                <div key={resource} className="bg-white" style={{ border: '1px solid #E8E8E8' }}>
                  <div className="px-4 py-3" style={{ borderBottom: '1px solid #E8E8E8', backgroundColor: '#FAFAFA' }}>
                    <span className="text-sm font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>
                      {resourceNames[resource] ?? resource}
                    </span>
                    <span className="ml-2 font-mono text-[9px] font-medium text-[#999999]">({perms.length} 项)</span>
                  </div>
                  {/* 表头 */}
                  <div className="flex items-center px-4 py-2" style={{ borderBottom: '1px solid #E8E8E8' }}>
                    {['权限名', '描述', '资源', '操作'].map((h) => (
                      <span key={h} className="flex-1 min-w-0 font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{h}</span>
                    ))}
                  </div>
                  {/* 数据行 */}
                  {perms.map((p, i) => (
                    <div key={p.id} className="flex items-center px-4 py-2.5" style={{ borderBottom: i < perms.length - 1 ? '1px solid #E8E8E8' : 'none' }}>
                      <span className="flex-1 min-w-0 font-mono text-[10px] font-medium text-[#1A1A1A]">{p.name}</span>
                      <span className="flex-1 min-w-0 font-mono text-[10px] font-medium text-[#999999]">{p.description}</span>
                      <div className="flex-1 min-w-0"><StatusTag label={p.resource} color="#0A6B3A" /></div>
                      <div className="flex-1 min-w-0"><StatusTag label={p.action} color="#D48700" /></div>
                    </div>
                  ))}
                </div>
              ))}
            </div>
          )}
        </>
      )}

      {/* 新建用户弹窗 */}
      <CreateUserDialog open={createDialogOpen} onClose={() => setCreateDialogOpen(false)} onCreated={loadUsers} />
    </div>
  )
}
