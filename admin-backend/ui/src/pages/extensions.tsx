import { Loader2 } from 'lucide-react'
import { useState, useEffect, useCallback, useMemo } from 'react'
import { api } from '@/lib/api'
import { TablePagination } from '@/components/ui/table-pagination'
import { ToggleSwitch } from '@/components/ui/toggle-switch'

/* ── 类型 ── */

interface ExtensionItem {
  id: string
  name: string
  description?: string
  version: string
  author?: string
  enabled: boolean
  updated_at: string
}

/* ── Tab 定义 ── */

const tabs = ['技能管理', '插件管理'] as const
type TabKey = typeof tabs[number]

/* ── 表格组件 ── */

function ExtensionTable({
  items,
  loading,
  onToggle,
  currentPage,
  onPageChange,
}: {
  items: ExtensionItem[]
  loading: boolean
  onToggle: (item: ExtensionItem) => void
  currentPage: number
  onPageChange: (page: number) => void
}) {
  const pageSize = 10
  const paged = useMemo(() => {
    const start = (currentPage - 1) * pageSize
    return items.slice(start, start + pageSize)
  }, [items, currentPage])

  return (
    <div className="bg-white" style={{ border: '1px solid #E8E8E8' }}>
      <div className="grid grid-cols-6 px-4 py-2.5" style={{ borderBottom: '1px solid #E8E8E8' }}>
        {['名称', '描述', '版本', '作者', '状态', '更新时间'].map((h) => (
          <span key={h} className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{h}</span>
        ))}
      </div>

      {loading ? (
        <div className="flex items-center justify-center py-10">
          <Loader2 className="h-5 w-5 animate-spin text-[#999999]" />
        </div>
      ) : paged.length === 0 ? (
        <div className="flex items-center justify-center py-10">
          <span className="font-mono text-[10px] text-[#CCCCCC]">暂无数据</span>
        </div>
      ) : (
        paged.map((s, i) => (
          <div key={s.id} className="grid grid-cols-6 items-center px-4 py-3" style={{ borderBottom: i < paged.length - 1 ? '1px solid #E8E8E8' : 'none' }}>
            <span className="font-mono text-[10px] font-semibold text-[#1A1A1A]">{s.name}</span>
            <span className="font-mono text-[10px] font-medium text-[#999999]">{s.description || '—'}</span>
            <span className="font-mono text-[10px] font-medium text-[#999999]">{s.version}</span>
            <span className="font-mono text-[10px] font-medium text-[#999999]">{s.author || '—'}</span>
            <div><ToggleSwitch on={s.enabled} onChange={() => onToggle(s)} /></div>
            <span className="font-mono text-[10px] font-medium text-[#999999]">
              {new Date(s.updated_at).toLocaleDateString('zh-CN')}
            </span>
          </div>
        ))
      )}
      <TablePagination current={currentPage} total={items.length} pageSize={pageSize} onChange={onPageChange} />
    </div>
  )
}

/* ── 主组件 ── */

export default function ExtensionsPage() {
  const [activeTab, setActiveTab] = useState<TabKey>('技能管理')
  const [currentPage, setCurrentPage] = useState(1)
  const [skills, setSkills] = useState<ExtensionItem[]>([])
  const [plugins, setPlugins] = useState<ExtensionItem[]>([])
  const [loadingSkills, setLoadingSkills] = useState(false)
  const [loadingPlugins, setLoadingPlugins] = useState(false)

  const loadSkills = useCallback(async () => {
    setLoadingSkills(true)
    try {
      const res = await api.get('/skills')
      setSkills(res.data.skills || [])
    } catch (err) {
      console.error('加载技能列表失败', err)
    } finally {
      setLoadingSkills(false)
    }
  }, [])

  const loadPlugins = useCallback(async () => {
    setLoadingPlugins(true)
    try {
      const res = await api.get('/plugins')
      setPlugins(res.data.plugins || [])
    } catch (err) {
      console.error('加载插件列表失败', err)
    } finally {
      setLoadingPlugins(false)
    }
  }, [])

  /* Tab 切换时加载对应数据 */
  useEffect(() => {
    if (activeTab === '技能管理') loadSkills()
    else loadPlugins()
  }, [activeTab, loadSkills, loadPlugins])

  const handleToggleSkill = useCallback(async (item: ExtensionItem) => {
    const action = item.enabled ? 'disable' : 'enable'
    try {
      await api.post(`/skills/${item.id}/${action}`)
      loadSkills()
    } catch (err) {
      console.error('切换技能状态失败', err)
    }
  }, [loadSkills])

  const handleTogglePlugin = useCallback(async (item: ExtensionItem) => {
    const action = item.enabled ? 'disable' : 'enable'
    try {
      await api.post(`/plugins/${item.id}/${action}`)
      loadPlugins()
    } catch (err) {
      console.error('切换插件状态失败', err)
    }
  }, [loadPlugins])

  const handleTabChange = useCallback((tab: TabKey) => {
    setActiveTab(tab)
    setCurrentPage(1)
  }, [])

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

      {/* Tab 内容区 */}
      {activeTab === '技能管理' && (
        <ExtensionTable
          items={skills}
          loading={loadingSkills}
          onToggle={handleToggleSkill}
          currentPage={currentPage}
          onPageChange={setCurrentPage}
        />
      )}
      {activeTab === '插件管理' && (
        <ExtensionTable
          items={plugins}
          loading={loadingPlugins}
          onToggle={handleTogglePlugin}
          currentPage={currentPage}
          onPageChange={setCurrentPage}
        />
      )}
    </div>
  )
}
