import { Link, useLocation } from 'react-router-dom'
import {
  LayoutDashboard, Monitor, Users, Shield, Bell,
  MessageSquare, BookOpen, Coins, ShieldCheck, Scan,
  SquareCheck, Settings, Building2, BarChart3, Bot,
  FileText, Puzzle,
} from 'lucide-react'
import { useAuthStore } from '@/stores/auth'

const navItems = [
  { title: '总览', icon: LayoutDashboard, href: '/' },
  { title: '客户端', icon: Monitor, href: '/clients' },
  { title: '用户管理', icon: Users, href: '/users' },
  { title: '部门管理', icon: Building2, href: '/departments' },
  { title: '安全策略', icon: Shield, href: '/security' },
  { title: '告警中心', icon: Bell, href: '/alerts' },
  { title: '对话审计', icon: MessageSquare, href: '/conversations' },
  { title: '知识库', icon: BookOpen, href: '/knowledge-bases' },
  { title: '模型配置', icon: Bot, href: '/model-configs' },
  { title: '扩展管理', icon: Puzzle, href: '/extensions' },
  { title: '合规管理', icon: ShieldCheck, href: '/compliance' },
  { title: '水印追踪', icon: Scan, href: '/watermark' },
  { title: '审批工单', icon: SquareCheck, href: '/approvals' },
  { title: '审计日志', icon: FileText, href: '/audit-logs' },
  { title: '统计报表', icon: BarChart3, href: '/reports' },
  { title: '系统设置', icon: Settings, href: '/settings' },
]

function isActive(pathname: string, href: string): boolean {
  if (href === '/') return pathname === '/'
  return pathname.startsWith(href)
}

export function AppSidebar() {
  const location = useLocation()
  const user = useAuthStore((s) => s.user)
  const logout = useAuthStore((s) => s.logout)

  return (
    <aside
      className="flex h-screen w-[240px] shrink-0 flex-col justify-between py-6"
      style={{ backgroundColor: '#FAFAFA', borderRight: '1px solid #E8E8E8' }}
    >
      {/* 上部 */}
      <div className="flex flex-col gap-8">
        {/* Logo */}
        <Link to="/" className="flex items-center gap-3 px-5">
          <div className="flex h-8 w-8 items-center justify-center bg-[#0A6B3A]">
            <span className="font-mono text-xs font-bold text-white">IC</span>
          </div>
          <span className="font-mono text-[13px] font-semibold tracking-[1px] text-[#1A1A1A]">
            IRONCLAW
          </span>
        </Link>

        {/* 系统状态卡片 */}
        <div
          className="mx-0 flex flex-col gap-2 bg-white p-5"
          style={{ borderTop: '1px solid #E8E8E8', borderRight: '1px solid #E8E8E8', borderBottom: '1px solid #E8E8E8' }}
        >
          {[
            { label: '运行时间:', value: '99.97%', color: '#0A6B3A' },
            { label: 'DLP状态:', value: '运行中', color: '#0A6B3A' },
            { label: '版本:', value: 'v1.0.0', color: '#1A1A1A' },
          ].map((row) => (
            <div key={row.label} className="flex justify-between">
              <span className="font-mono text-[10px] font-medium text-[#999999]">{row.label}</span>
              <span className="font-mono text-[10px] font-semibold" style={{ color: row.color }}>{row.value}</span>
            </div>
          ))}
        </div>

        {/* 导航菜单 */}
        <nav className="flex flex-col gap-0.5">
          {navItems.map((item) => {
            const active = isActive(location.pathname, item.href)
            return (
              <Link
                key={item.href}
                to={item.href}
                className="flex items-center gap-3 px-5 py-3"
                style={{
                  backgroundColor: active ? 'rgba(10,107,58,0.06)' : 'transparent',
                  borderLeft: active ? '2px solid #0A6B3A' : '2px solid transparent',
                }}
              >
                <item.icon
                  className="h-4 w-4"
                  style={{ color: active ? '#0A6B3A' : '#6a6a6a' }}
                />
                <span
                  className="font-mono text-[11px] tracking-[0.5px]"
                  style={{
                    color: active ? '#1A1A1A' : '#999999',
                    fontWeight: active ? 600 : 500,
                  }}
                >
                  {item.title}
                </span>
              </Link>
            )
          })}
        </nav>
      </div>

      {/* 底部用户信息 */}
      <div className="px-5 pt-4" style={{ borderTop: '1px solid #E8E8E8' }}>
        <button onClick={logout} className="flex w-full items-center gap-3" title="点击退出登录">
          <div
            className="flex h-9 w-9 shrink-0 items-center justify-center"
            style={{ backgroundColor: '#E8E8E8', border: '1px solid #D9D9D9' }}
          >
            <span className="font-mono text-[10px] font-semibold text-[#0A6B3A]">
              {user?.username?.charAt(0) ?? '管'}
            </span>
          </div>
          <div className="flex flex-col gap-0.5 text-left">
            <span className="font-mono text-[11px] font-semibold text-[#1A1A1A]">
              {user?.username ?? '管理员'}
            </span>
            <span className="font-mono text-[9px] font-medium text-[#999999]">
              {user?.role ?? '超级管理员'}
            </span>
          </div>
        </button>
      </div>
    </aside>
  )
}
