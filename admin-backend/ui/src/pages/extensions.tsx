import { useState } from 'react'

/* ── Mock 数据 ── */

const tabs = ['技能管理', '插件管理']

const mockSkills = [
  { name: '网页搜索', description: '搜索互联网获取最新信息', version: 'v1.2.0', author: 'IronClaw', enabled: true, updatedAt: '2024-07-08' },
  { name: '代码执行', description: '在沙箱环境中执行代码片段', version: 'v2.0.1', author: 'IronClaw', enabled: true, updatedAt: '2024-07-05' },
  { name: '文件分析', description: '解析和分析上传的文档内容', version: 'v1.5.3', author: 'IronClaw', enabled: true, updatedAt: '2024-07-01' },
  { name: '图片生成', description: '基于文本描述生成图片', version: 'v0.9.0', author: 'Community', enabled: false, updatedAt: '2024-06-28' },
]

/* ── Toggle 组件 ── */

function Toggle({ on }: { on: boolean }) {
  return (
    <div
      className="relative h-5 w-9 cursor-pointer"
      style={{ backgroundColor: on ? '#0A6B3A' : '#D9D9D9', borderRadius: 10 }}
    >
      <div
        className="absolute top-0.5 h-4 w-4 bg-white"
        style={{ borderRadius: 8, left: on ? 18 : 2, transition: 'left 0.2s' }}
      />
    </div>
  )
}

/* ── 组件 ── */

export default function ExtensionsPage() {
  const [activeTab, setActiveTab] = useState('技能管理')

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

      {/* 表格 */}
      <div className="bg-white" style={{ border: '1px solid #E8E8E8' }}>
        {/* 表头 */}
        <div className="grid grid-cols-6 px-4 py-2.5" style={{ borderBottom: '1px solid #E8E8E8' }}>
          {['名称', '描述', '版本', '作者', '状态', '更新时间'].map((h) => (
            <span key={h} className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{h}</span>
          ))}
        </div>

        {/* 数据行 */}
        {mockSkills.map((s, i) => (
          <div
            key={s.name}
            className="grid grid-cols-6 items-center px-4 py-3"
            style={{ borderBottom: i < mockSkills.length - 1 ? '1px solid #E8E8E8' : 'none' }}
          >
            <span className="font-mono text-[10px] font-semibold text-[#1A1A1A]">{s.name}</span>
            <span className="font-mono text-[10px] font-medium text-[#999999]">{s.description}</span>
            <span className="font-mono text-[10px] font-medium text-[#999999]">{s.version}</span>
            <span className="font-mono text-[10px] font-medium text-[#999999]">{s.author}</span>
            <div><Toggle on={s.enabled} /></div>
            <span className="font-mono text-[10px] font-medium text-[#999999]">{s.updatedAt}</span>
          </div>
        ))}
      </div>
    </div>
  )
}
