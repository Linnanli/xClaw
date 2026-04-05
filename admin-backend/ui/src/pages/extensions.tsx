import { Loader2, Upload, CheckCircle, XCircle, Clock } from 'lucide-react'
import { useState, useEffect, useCallback, useMemo } from 'react'
import { api } from '@/lib/api'
import { TablePagination } from '@/components/ui/table-pagination'
import { ToggleSwitch } from '@/components/ui/toggle-switch'

/* ── 类型 ── */

interface SkillItem {
  id: string
  name: string
  description?: string
  version: string
  author?: string
  enabled: boolean
  source: 'builtin' | 'admin_upload'
  review_status: 'pending' | 'approved' | 'rejected'
  is_builtin: boolean
  invoke_count: number
  review_note?: string
  updated_at: string
}

interface PluginItem {
  id: string
  name: string
  description?: string
  version: string
  author?: string
  enabled: boolean
  source: 'builtin' | 'admin_upload'
  review_status: 'pending' | 'approved' | 'rejected'
  plugin_type: 'http' | 'stdio' | 'wasm'
  is_builtin: boolean
  invoke_count: number
  requires_sandbox: boolean
  review_note?: string
  updated_at: string
}

/* ── 徽标组件 ── */

function SourceBadge({ source }: { source: string }) {
  const isBuiltin = source === 'builtin'
  return (
    <span className="font-mono text-[9px] px-1.5 py-0.5 rounded"
      style={{ background: isBuiltin ? '#F0F0F0' : '#E8F5E9', color: isBuiltin ? '#666' : '#0A6B3A' }}>
      {isBuiltin ? '内置' : '上传'}
    </span>
  )
}

function ReviewBadge({ status }: { status: string }) {
  const map: Record<string, { label: string; color: string; bg: string; Icon: typeof Clock }> = {
    approved: { label: '已通过', color: '#0A6B3A', bg: '#E8F5E9', Icon: CheckCircle },
    pending:  { label: '待审核', color: '#B45309', bg: '#FEF3C7', Icon: Clock },
    rejected: { label: '已拒绝', color: '#DC2626', bg: '#FEE2E2', Icon: XCircle },
  }
  const cfg = map[status] ?? map.pending
  const { label, color, bg, Icon } = cfg
  return (
    <span className="inline-flex items-center gap-1 font-mono text-[9px] px-1.5 py-0.5 rounded"
      style={{ background: bg, color }}>
      <Icon size={10} />
      {label}
    </span>
  )
}

/* ── 上传弹窗 ── */

function UploadSkillModal({ onClose, onSuccess }: { onClose: () => void; onSuccess: () => void }) {
  const [name, setName] = useState('')
  const [content, setContent] = useState('')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState('')

  const handleSubmit = async () => {
    if (!name.trim() || !content.trim()) { setError('名称和内容不能为空'); return }
    setLoading(true); setError('')
    try {
      await api.post('/skills/upload', { name, content })
      onSuccess()
      onClose()
    } catch (e: any) {
      setError(e?.response?.data?.message || '上传失败')
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/30">
      <div className="bg-white w-[520px] rounded-lg p-6 flex flex-col gap-4" style={{ border: '1px solid #E8E8E8' }}>
        <h2 className="font-mono text-sm font-bold text-[#1A1A1A]">上传技能包</h2>
        <div className="flex flex-col gap-1">
          <label className="font-mono text-[10px] text-[#999]">技能名称</label>
          <input value={name} onChange={e => setName(e.target.value)}
            className="border border-[#E8E8E8] rounded px-3 py-2 font-mono text-xs outline-none focus:border-[#0A6B3A]" />
        </div>
        <div className="flex flex-col gap-1">
          <label className="font-mono text-[10px] text-[#999]">SKILL.md 内容</label>
          <textarea value={content} onChange={e => setContent(e.target.value)} rows={8}
            placeholder="---&#10;name: my-skill&#10;description: ...&#10;---&#10;&#10;# 技能内容"
            className="border border-[#E8E8E8] rounded px-3 py-2 font-mono text-xs outline-none focus:border-[#0A6B3A] resize-none" />
        </div>
        {error && <p className="font-mono text-[10px] text-red-500">{error}</p>}
        <div className="flex justify-end gap-2">
          <button onClick={onClose} className="font-mono text-[10px] px-4 py-2 border border-[#E8E8E8] rounded text-[#666]">取消</button>
          <button onClick={handleSubmit} disabled={loading}
            className="font-mono text-[10px] px-4 py-2 rounded text-white flex items-center gap-1"
            style={{ background: '#0A6B3A' }}>
            {loading && <Loader2 size={12} className="animate-spin" />}
            上传
          </button>
        </div>
      </div>
    </div>
  )
}

/* ── 审核弹窗 ── */

function ReviewModal({
  id, name, type: itemType, onClose, onSuccess,
}: { id: string; name: string; type: 'skill' | 'plugin'; onClose: () => void; onSuccess: () => void }) {
  const [note, setNote] = useState('')
  const [loading, setLoading] = useState(false)

  const submit = async (approved: boolean) => {
    setLoading(true)
    try {
      await api.post(`/${itemType === 'skill' ? 'skills' : 'plugins'}/${id}/review`, { approved, note })
      onSuccess()
      onClose()
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/30">
      <div className="bg-white w-[400px] rounded-lg p-6 flex flex-col gap-4" style={{ border: '1px solid #E8E8E8' }}>
        <h2 className="font-mono text-sm font-bold text-[#1A1A1A]">审核：{name}</h2>
        <div className="flex flex-col gap-1">
          <label className="font-mono text-[10px] text-[#999]">审核备注（可选）</label>
          <textarea value={note} onChange={e => setNote(e.target.value)} rows={3}
            className="border border-[#E8E8E8] rounded px-3 py-2 font-mono text-xs outline-none focus:border-[#0A6B3A] resize-none" />
        </div>
        <div className="flex justify-end gap-2">
          <button onClick={onClose} className="font-mono text-[10px] px-4 py-2 border border-[#E8E8E8] rounded text-[#666]">取消</button>
          <button onClick={() => submit(false)} disabled={loading}
            className="font-mono text-[10px] px-4 py-2 rounded text-white" style={{ background: '#DC2626' }}>
            拒绝
          </button>
          <button onClick={() => submit(true)} disabled={loading}
            className="font-mono text-[10px] px-4 py-2 rounded text-white flex items-center gap-1" style={{ background: '#0A6B3A' }}>
            {loading && <Loader2 size={12} className="animate-spin" />}
            通过
          </button>
        </div>
      </div>
    </div>
  )
}

/* ── 技能表格 ── */

function SkillTable({
  items, loading, onToggle, onReview, currentPage, onPageChange,
}: {
  items: SkillItem[]
  loading: boolean
  onToggle: (item: SkillItem) => void
  onReview: (item: SkillItem) => void
  currentPage: number
  onPageChange: (page: number) => void
}) {
  const pageSize = 10
  const paged = useMemo(() => {
    const start = (currentPage - 1) * pageSize
    return items.slice(start, start + pageSize)
  }, [items, currentPage])

  const headers = ['名称', '描述', '来源', '审核状态', '版本', '调用次数', '状态', '操作']

  return (
    <div className="bg-white" style={{ border: '1px solid #E8E8E8' }}>
      <div className="grid grid-cols-8 px-4 py-2.5" style={{ borderBottom: '1px solid #E8E8E8' }}>
        {headers.map(h => (
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
          <div key={s.id} className="grid grid-cols-8 items-center px-4 py-3"
            style={{ borderBottom: i < paged.length - 1 ? '1px solid #E8E8E8' : 'none' }}>
            <div>
              <span className="font-mono text-[10px] font-semibold text-[#1A1A1A]">{s.name}</span>
              {s.is_builtin && (
                <span className="ml-1 font-mono text-[8px] px-1 rounded" style={{ background: '#F0F0F0', color: '#666' }}>内置</span>
              )}
            </div>
            <span className="font-mono text-[10px] text-[#999] truncate pr-2" title={s.description}>{s.description || '—'}</span>
            <SourceBadge source={s.source} />
            <ReviewBadge status={s.review_status} />
            <span className="font-mono text-[10px] text-[#999]">{s.version}</span>
            <span className="font-mono text-[10px] text-[#999]">{s.invoke_count.toLocaleString()}</span>
            <div><ToggleSwitch on={s.enabled} onChange={() => onToggle(s)} /></div>
            <div>
              {s.review_status === 'pending' && (
                <button onClick={() => onReview(s)}
                  className="font-mono text-[9px] px-2 py-1 rounded border border-[#E8E8E8] text-[#0A6B3A] hover:bg-[#F0FFF4]">
                  审核
                </button>
              )}
            </div>
          </div>
        ))
      )}
      <TablePagination current={currentPage} total={items.length} pageSize={pageSize} onChange={onPageChange} />
    </div>
  )
}

/* ── 插件表格 ── */

function PluginTable({
  items, loading, onToggle, onReview, currentPage, onPageChange,
}: {
  items: PluginItem[]
  loading: boolean
  onToggle: (item: PluginItem) => void
  onReview: (item: PluginItem) => void
  currentPage: number
  onPageChange: (page: number) => void
}) {
  const pageSize = 10
  const paged = useMemo(() => {
    const start = (currentPage - 1) * pageSize
    return items.slice(start, start + pageSize)
  }, [items, currentPage])

  const headers = ['名称', '描述', '类型', '审核状态', '版本', '沙箱', '状态', '操作']

  return (
    <div className="bg-white" style={{ border: '1px solid #E8E8E8' }}>
      <div className="grid grid-cols-8 px-4 py-2.5" style={{ borderBottom: '1px solid #E8E8E8' }}>
        {headers.map(h => (
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
        paged.map((p, i) => (
          <div key={p.id} className="grid grid-cols-8 items-center px-4 py-3"
            style={{ borderBottom: i < paged.length - 1 ? '1px solid #E8E8E8' : 'none' }}>
            <span className="font-mono text-[10px] font-semibold text-[#1A1A1A]">{p.name}</span>
            <span className="font-mono text-[10px] text-[#999] truncate pr-2" title={p.description}>{p.description || '—'}</span>
            <span className="font-mono text-[9px] px-1.5 py-0.5 rounded uppercase"
              style={{ background: '#F0F0F0', color: '#666' }}>{p.plugin_type}</span>
            <ReviewBadge status={p.review_status} />
            <span className="font-mono text-[10px] text-[#999]">{p.version}</span>
            <span className="font-mono text-[9px]" style={{ color: p.requires_sandbox ? '#B45309' : '#999' }}>
              {p.requires_sandbox ? '需要' : '—'}
            </span>
            <div><ToggleSwitch on={p.enabled} onChange={() => onToggle(p)} /></div>
            <div>
              {p.review_status === 'pending' && (
                <button onClick={() => onReview(p)}
                  className="font-mono text-[9px] px-2 py-1 rounded border border-[#E8E8E8] text-[#0A6B3A] hover:bg-[#F0FFF4]">
                  审核
                </button>
              )}
            </div>
          </div>
        ))
      )}
      <TablePagination current={currentPage} total={items.length} pageSize={pageSize} onChange={onPageChange} />
    </div>
  )
}

/* ── 主组件 ── */

const tabs = ['技能管理', '插件管理'] as const
type TabKey = typeof tabs[number]

export default function ExtensionsPage() {
  const [activeTab, setActiveTab] = useState<TabKey>('技能管理')
  const [currentPage, setCurrentPage] = useState(1)
  const [skills, setSkills] = useState<SkillItem[]>([])
  const [plugins, setPlugins] = useState<PluginItem[]>([])
  const [loadingSkills, setLoadingSkills] = useState(false)
  const [loadingPlugins, setLoadingPlugins] = useState(false)
  const [showUploadModal, setShowUploadModal] = useState(false)
  const [reviewTarget, setReviewTarget] = useState<{ id: string; name: string; type: 'skill' | 'plugin' } | null>(null)

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

  useEffect(() => {
    if (activeTab === '技能管理') loadSkills()
    else loadPlugins()
  }, [activeTab, loadSkills, loadPlugins])

  const handleToggleSkill = useCallback(async (item: SkillItem) => {
    const action = item.enabled ? 'disable' : 'enable'
    try {
      await api.post(`/skills/${item.id}/${action}`)
      loadSkills()
    } catch (err) {
      console.error('切换技能状态失败', err)
    }
  }, [loadSkills])

  const handleTogglePlugin = useCallback(async (item: PluginItem) => {
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

  const pendingSkills = useMemo(() => skills.filter(s => s.review_status === 'pending').length, [skills])
  const pendingPlugins = useMemo(() => plugins.filter(p => p.review_status === 'pending').length, [plugins])

  return (
    <div className="flex flex-col gap-6">
      <div className="flex items-end justify-between">
        <div className="flex flex-col gap-2">
          <h1 className="text-[22px] font-bold tracking-tight text-[#1A1A1A]"
            style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px' }}>
            扩展管理
          </h1>
          <p className="font-mono text-xs text-[#999999]">管理 AI 助手可用的技能和插件</p>
        </div>
        {activeTab === '技能管理' && (
          <button onClick={() => setShowUploadModal(true)}
            className="flex items-center gap-2 font-mono text-[10px] px-4 py-2 rounded text-white"
            style={{ background: '#0A6B3A' }}>
            <Upload size={12} />
            上传技能包
          </button>
        )}
      </div>

      <div className="flex gap-0" style={{ borderBottom: '1px solid #E8E8E8' }}>
        {tabs.map((tab) => {
          const pending = tab === '技能管理' ? pendingSkills : pendingPlugins
          return (
            <button key={tab} onClick={() => handleTabChange(tab)}
              className="relative px-5 py-3 font-mono text-[10px] font-semibold"
              style={{
                color: activeTab === tab ? '#0A6B3A' : '#999999',
                borderBottom: activeTab === tab ? '2px solid #0A6B3A' : '2px solid transparent',
              }}>
              {tab}
              {pending > 0 && (
                <span className="absolute -top-1 -right-1 w-4 h-4 rounded-full text-white font-mono text-[8px] flex items-center justify-center"
                  style={{ background: '#B45309' }}>{pending}</span>
              )}
            </button>
          )
        })}
      </div>

      {activeTab === '技能管理' && (
        <SkillTable
          items={skills}
          loading={loadingSkills}
          onToggle={handleToggleSkill}
          onReview={s => setReviewTarget({ id: s.id, name: s.name, type: 'skill' })}
          currentPage={currentPage}
          onPageChange={setCurrentPage}
        />
      )}
      {activeTab === '插件管理' && (
        <PluginTable
          items={plugins}
          loading={loadingPlugins}
          onToggle={handleTogglePlugin}
          onReview={p => setReviewTarget({ id: p.id, name: p.name, type: 'plugin' })}
          currentPage={currentPage}
          onPageChange={setCurrentPage}
        />
      )}

      {showUploadModal && (
        <UploadSkillModal
          onClose={() => setShowUploadModal(false)}
          onSuccess={loadSkills}
        />
      )}

      {reviewTarget && (
        <ReviewModal
          id={reviewTarget.id}
          name={reviewTarget.name}
          type={reviewTarget.type}
          onClose={() => setReviewTarget(null)}
          onSuccess={reviewTarget.type === 'skill' ? loadSkills : loadPlugins}
        />
      )}
    </div>
  )
}
