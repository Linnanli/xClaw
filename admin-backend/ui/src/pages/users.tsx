import { Search, Upload, RefreshCw, Plus, Pencil, Trash2, ChevronDown } from 'lucide-react'
import { useState } from 'react'
import { TablePagination } from '@/components/ui/table-pagination'

/* ── Mock 数据 ── */

const mockUsers = [
  { username: 'zhang.wei', email: 'zhang.wei@ironclaw.com', role: '超级管理员', roleColor: '#0A6B3A', department: '研发部', mfa: true, status: 'active' as const },
  { username: 'li.ming', email: 'li.ming@ironclaw.com', role: '管理员', roleColor: '#1677FF', department: '产品部', mfa: true, status: 'active' as const },
  { username: 'wang.fang', email: 'wang.fang@ironclaw.com', role: '普通用户', roleColor: '#999999', department: '市场部', mfa: false, status: 'active' as const },
  { username: 'chen.jing', email: 'chen.jing@ironclaw.com', role: '普通用户', roleColor: '#999999', department: '运营部', mfa: false, status: 'disabled' as const },
]

const mockRoles = [
  { name: '超级管理员', users: 2, permissions: 48, description: '拥有系统全部权限', color: '#0A6B3A' },
  { name: '管理员', users: 5, permissions: 32, description: '管理用户和基础配置', color: '#1677FF' },
  { name: '审计员', users: 3, permissions: 12, description: '查看审计日志和报表', color: '#D48700' },
  { name: '普通用户', users: 79, permissions: 8, description: '基础使用权限', color: '#999999' },
]

const mockPermissions = [
  { module: '用户管理', read: true, write: true, delete: true, admin: true },
  { module: 'DLP 规则', read: true, write: true, delete: false, admin: false },
  { module: '审计日志', read: true, write: false, delete: false, admin: false },
  { module: '客户端管理', read: true, write: true, delete: true, admin: false },
]

const tabs = ['用户列表', '角色管理', '权限管理']

/* ── 组件 ── */

export default function UsersPage() {
  const [activeTab, setActiveTab] = useState('用户列表')
  const [currentPage, setCurrentPage] = useState(1)

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
          <button className="flex items-center gap-2 bg-[#0A6B3A] px-4 py-2.5 font-mono text-[9px] font-semibold text-white">
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
            onClick={() => setActiveTab(tab)}
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

      {/* Tab 内容区 */}
      {activeTab === '用户列表' && (
        <>
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <div className="flex w-[220px] items-center gap-2.5 border border-[#E8E8E8] bg-white px-3.5 py-2">
                <Search className="h-3 w-3 text-[#6a6a6a]" />
                <span className="font-mono text-[10px] font-medium text-[#CCCCCC]">搜索用户名、邮箱...</span>
              </div>
              <div className="flex items-center gap-2 border border-[#E8E8E8] bg-white px-3.5 py-2">
                <span className="font-mono text-[10px] font-medium text-[#999999]">角色</span>
                <ChevronDown className="h-3 w-3 text-[#999999]" />
              </div>
              <div className="flex items-center gap-2 border border-[#E8E8E8] bg-white px-3.5 py-2">
                <span className="font-mono text-[10px] font-medium text-[#999999]">部门</span>
                <ChevronDown className="h-3 w-3 text-[#999999]" />
              </div>
            </div>
            <span className="font-mono text-[10px] font-medium text-[#999999]">共 89 个用户</span>
          </div>
          <div className="bg-white" style={{ border: '1px solid #E8E8E8' }}>
            <div className="grid grid-cols-7 px-4 py-2.5" style={{ borderBottom: '1px solid #E8E8E8' }}>
              {['用户名', '邮箱', '角色', '部门', 'MFA', '状态', '操作'].map((h) => (
                <span key={h} className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{h}</span>
              ))}
            </div>
            {mockUsers.map((u, i) => (
              <div key={u.username} className="grid grid-cols-7 items-center px-4 py-3" style={{ borderBottom: i < mockUsers.length - 1 ? '1px solid #E8E8E8' : 'none' }}>
                <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{u.username}</span>
                <span className="font-mono text-[10px] font-medium text-[#999999]">{u.email}</span>
                <div>
                  <span className="inline-block px-2 py-0.5 font-mono text-[9px] font-semibold" style={{ color: u.roleColor, backgroundColor: `${u.roleColor}1A`, border: `1px solid ${u.roleColor}` }}>
                    {u.role}
                  </span>
                </div>
                <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{u.department}</span>
                <span className="font-mono text-[10px] font-medium" style={{ color: u.mfa ? '#0A6B3A' : '#CCCCCC' }}>{u.mfa ? '✓' : '—'}</span>
                <div className="flex items-center gap-1.5">
                  <div className="h-2 w-2 rounded-full" style={{ backgroundColor: u.status === 'active' ? '#0A6B3A' : '#CF1322' }} />
                  <span className="font-mono text-[10px] font-medium" style={{ color: u.status === 'active' ? '#1A1A1A' : '#CF1322' }}>{u.status === 'active' ? '启用' : '禁用'}</span>
                </div>
                <div className="flex items-center gap-3">
                  <button className="text-[#6a6a6a] hover:text-[#0A6B3A]" title="编辑"><Pencil className="h-3.5 w-3.5" /></button>
                  <button className="text-[#6a6a6a] hover:text-[#CF1322]" title="删除"><Trash2 className="h-3.5 w-3.5" /></button>
                </div>
              </div>
            ))}
            <TablePagination current={currentPage} total={89} pageSize={10} onChange={setCurrentPage} />
          </div>
        </>
      )}

      {activeTab === '角色管理' && (
        <div className="grid grid-cols-2 gap-4">
          {mockRoles.map((r) => (
            <div key={r.name} className="flex flex-col gap-3 bg-white p-5" style={{ border: '1px solid #E8E8E8' }}>
              <div className="flex items-center justify-between">
                <span className="text-sm font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>{r.name}</span>
                <span className="inline-block px-2 py-0.5 font-mono text-[9px] font-semibold" style={{ color: r.color, backgroundColor: `${r.color}1A`, border: `1px solid ${r.color}` }}>{r.users} 人</span>
              </div>
              <span className="font-mono text-[10px] font-medium text-[#999999]">{r.description}</span>
              <div className="flex items-center justify-between">
                <span className="font-mono text-[10px] font-medium text-[#999999]">{r.permissions} 项权限</span>
                <button className="font-mono text-[10px] font-semibold text-[#0A6B3A]">编辑</button>
              </div>
            </div>
          ))}
        </div>
      )}

      {activeTab === '权限管理' && (
        <div className="bg-white" style={{ border: '1px solid #E8E8E8' }}>
          <div className="grid grid-cols-5 px-4 py-2.5" style={{ borderBottom: '1px solid #E8E8E8' }}>
            {['功能模块', '查看', '编辑', '删除', '管理'].map((h) => (
              <span key={h} className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{h}</span>
            ))}
          </div>
          {mockPermissions.map((p, i) => (
            <div key={p.module} className="grid grid-cols-5 items-center px-4 py-3" style={{ borderBottom: i < mockPermissions.length - 1 ? '1px solid #E8E8E8' : 'none' }}>
              <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{p.module}</span>
              {[p.read, p.write, p.delete, p.admin].map((v, j) => (
                <span key={j} className="font-mono text-[10px] font-medium" style={{ color: v ? '#0A6B3A' : '#CCCCCC' }}>{v ? '✓' : '—'}</span>
              ))}
            </div>
          ))}
          <TablePagination current={currentPage} total={12} pageSize={10} onChange={setCurrentPage} />
        </div>
      )}
    </div>
  )
}
