import { useState } from 'react'
import { TablePagination } from '@/components/ui/table-pagination'
import { ToggleSwitch } from '@/components/ui/toggle-switch'

/* ── Mock 数据 ── */

const tabs = ['技能管理', '插件管理']

const mockSkills = [
  { name: '网页搜索', description: '搜索互联网获取最新信息', version: 'v1.2.0', author: 'IronClaw', enabled: true, updatedAt: '2024-07-08' },
  { name: '代码执行', description: '在沙箱环境中执行代码片段', version: 'v2.0.1', author: 'IronClaw', enabled: true, updatedAt: '2024-07-05' },
  { name: '文件分析', description: '解析和分析上传的文档内容', version: 'v1.5.3', author: 'IronClaw', enabled: true, updatedAt: '2024-07-01' },
  { name: '图片生成', description: '基于文本描述生成图片', version: 'v0.9.0', author: 'Community', enabled: false, updatedAt: '2024-06-28' },
]

const mockPlugins = [
  { name: 'Slack 集成', description: '将 AI 助手接入 Slack 工作区', version: 'v1.0.2', author: 'IronClaw', enabled: true, updatedAt: '2024-07-06' },
  { name: '飞书集成', description: '将 AI 助手接入飞书', version: 'v0.8.1', author: 'Community', enabled: false, updatedAt: '2024-06-25' },
  { name: 'Jira 集成', description: '自动创建和更新 Jira 工单', version: 'v1.1.0', author: 'IronClaw', enabled: true, updatedAt: '2024-07-03' },
]

/* ── 表格组件 ── */

function ExtensionTable({ data, total, currentPage, onPageChange }: { data: typeof mockSkills; total: number; currentPage: number; onPageChange: (page: number) => void }) {
  return (
    <div className="bg-white" style={{ border: '1px solid #E8E8E8' }}>
      <div className="grid grid-cols-6 px-4 py-2.5" style={{ borderBottom: '1px solid #E8E8E8' }}>
        {['名称', '描述', '版本', '作者', '状态', '更新时间'].map((h) => (
          <span key={h} className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{h}</span>
        ))}
      </div>
      {data.map((s, i) => (
        <div key={s.name} className="grid grid-cols-6 items-center px-4 py-3" style={{ borderBottom: i < data.length - 1 ? '1px solid #E8E8E8' : 'none' }}>
          <span className="font-mono text-[10px] font-semibold text-[#1A1A1A]">{s.name}</span>
          <span className="font-mono text-[10px] font-medium text-[#999999]">{s.description}</span>
          <span className="font-mono text-[10px] font-medium text-[#999999]">{s.version}</span>
          <span className="font-mono text-[10px] font-medium text-[#999999]">{s.author}</span>
          <div><ToggleSwitch on={s.enabled} /></div>
          <span className="font-mono text-[10px] font-medium text-[#999999]">{s.updatedAt}</span>
        </div>
      ))}
      <TablePagination current={currentPage} total={total} pageSize={10} onChange={onPageChange} />
    </div>
  )
}

/* ── 组件 ── */

export default function ExtensionsPage() {
  const [activeTab, setActiveTab] = useState('技能管理')
  const [currentPage, setCurrentPage] = useState(1)

  return (
    <div className="flex flex-col gap-6">

      {/* 标题区 */}
      <div className="flex items-end justify-between">
        <div className="flex flex-col gap-2">
          <h1 className="text-[22px] font-bold tracking-tight text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px' }}>
            扩展管理
          </h1>
          <p className="font-mono text-xs text-[#999999]">管理 AI 助手可用的技能和插件</p>
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
      {activeTab === '技能管理' && <ExtensionTable data={mockSkills} total={12} currentPage={currentPage} onPageChange={setCurrentPage} />}
      {activeTab === '插件管理' && <ExtensionTable data={mockPlugins} total={8} currentPage={currentPage} onPageChange={setCurrentPage} />}
    </div>
  )
}
