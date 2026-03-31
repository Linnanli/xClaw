import { Outlet, useLocation } from 'react-router-dom'
import { AppSidebar } from './app-sidebar'
import { Search, Bell } from 'lucide-react'

const routeTitles: Record<string, string> = {
  '/': '总览',
  '/clients': '客户端管理',
  '/users': '用户管理',
  '/security': '安全策略',
  '/alerts': '告警中心',
  '/conversations': '对话审计',
  '/knowledge-bases': '知识库',
  '/compliance': '合规管理',
  '/watermark': '水印追踪',
  '/approvals': '审批工单',
  '/settings': '系统设置',
  '/audit-logs': '审计日志',
  '/reports': '统计报表',
  '/departments': '部门管理',
}

function getPageTitle(pathname: string): string {
  if (routeTitles[pathname]) return routeTitles[pathname]
  for (const [route, title] of Object.entries(routeTitles)) {
    if (route !== '/' && pathname.startsWith(route)) return title
  }
  return '页面'
}

export function AppLayout() {
  const location = useLocation()
  const pageTitle = getPageTitle(location.pathname)

  return (
    <div className="flex h-screen w-screen overflow-hidden">
      <AppSidebar />
      <div className="flex flex-1 flex-col overflow-hidden">
        {/* 顶部栏 */}
        <header className="flex h-14 shrink-0 items-center justify-between border-b border-[#E8E8E8] px-8">
          {/* 面包屑 */}
          <div className="flex items-center gap-2">
            <span className="font-mono text-[10px] font-medium text-[#999999]">系统</span>
            <span className="font-mono text-[10px] font-medium text-[#CCCCCC]">&gt;</span>
            <span className="font-mono text-[10px] font-semibold text-[#0A6B3A]">{pageTitle}</span>
          </div>

          {/* 右侧：搜索 + 告警 */}
          <div className="flex items-center gap-3">
            <div className="flex w-[200px] items-center gap-2.5 border border-[#E8E8E8] bg-white px-3.5 py-2">
              <Search className="h-3 w-3 text-[#6a6a6a]" />
              <span className="font-mono text-[10px] font-medium text-[#CCCCCC]">搜索...</span>
            </div>
            <div
              className="flex h-8 w-9 items-center justify-center"
              style={{ border: '1px solid #CF1322' }}
            >
              <Bell className="h-3.5 w-3.5 text-[#CF1322]" />
            </div>
          </div>
        </header>

        {/* 内容区 */}
        <main className="flex-1 overflow-auto px-8 py-7">
          <Outlet />
        </main>
      </div>
    </div>
  )
}
