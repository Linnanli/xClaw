import { Search, Plus, RefreshCw, Pencil, Trash2, Star, EyeOff, ChevronDown, Bot, Link, Plug, Check, Loader2 } from 'lucide-react'
import { useState, useEffect, useCallback } from 'react'
import { TablePagination } from '@/components/ui/table-pagination'
import { ToggleSwitch } from '@/components/ui/toggle-switch'
import { Dialog, DialogContent } from '@/components/ui/dialog'
import { api } from '@/lib/api'

/* ── 类型定义 ── */

interface ModelConfig {
  id: string
  model_id: string
  display_name: string
  description: string | null
  provider: string
  api_base_url: string | null
  api_key: string | null
  enabled: boolean
  is_default: boolean
  sort_order: number
  capabilities: string[]
  extra_config: Record<string, unknown>
  created_at: string
  updated_at: string
}

/* ── 提供商配置 ── */

interface ProviderConfig {
  value: string
  label: string
  icon: string
  color: string
  baseUrl: string
}

const PROVIDER_LIST: ProviderConfig[] = [
  { value: 'deepseek',   label: 'DeepSeek',   icon: '🐋', color: '#1677FF', baseUrl: 'https://api.deepseek.com' },
  { value: 'moonshot',   label: 'Moonshot',   icon: '🌙', color: '#722ed1', baseUrl: 'https://api.moonshot.cn/v1' },
  { value: 'qwen',       label: 'Qwen',       icon: '✦',  color: '#9254de', baseUrl: 'https://dashscope.aliyuncs.com/compatible-mode/v1' },
  { value: 'zhipu',      label: 'Zhipu',      icon: '⬡',  color: '#2f54eb', baseUrl: 'https://open.bigmodel.cn/api/paas/v4' },
  { value: 'minimax',    label: 'MiniMax',    icon: '〰', color: '#f5222d', baseUrl: 'https://api.minimaxi.com/v1' },
  { value: 'volcengine', label: 'Volcengine', icon: '🔥', color: '#13c2c2', baseUrl: 'https://ark.cn-beijing.volces.com/api/v3' },
  { value: 'ollama',     label: 'Ollama',     icon: '🦙', color: '#595959', baseUrl: 'http://localhost:11434/v1' },
  { value: 'openai',     label: 'OpenAI',     icon: '◎',  color: '#52c41a', baseUrl: 'https://api.openai.com/v1' },
  { value: 'anthropic',  label: 'Anthropic',  icon: '◈',  color: '#722ed1', baseUrl: 'https://api.anthropic.com/v1' },
  { value: 'custom',     label: 'Custom',     icon: '✏',  color: '#fa8c16', baseUrl: '' },
]

const PROVIDERS = Object.fromEntries(PROVIDER_LIST.map((p) => [p.value, p]))

/* ── API Key 脱敏 ── */

function maskApiKey(key: string | null): string {
  if (!key) return '—'
  if (key.length <= 8) return '••••••••'
  return `${key.slice(0, 4)}••••${key.slice(-4)}`
}

/* ── 辅助组件 ── */

function ProviderTag({ provider }: { provider: string }) {
  const p = PROVIDERS[provider]
  if (!p) return <span className="font-mono text-[10px] text-[#999999]">{provider}</span>
  return (
    <span className="inline-flex items-center gap-1 px-2 py-0.5 font-mono text-[8px] font-semibold" style={{ color: p.color, backgroundColor: `${p.color}15`, border: `1px solid ${p.color}` }}>
      {p.icon} {p.label}
    </span>
  )
}

function CapabilityTag({ label }: { label: string }) {
  return (
    <span className="inline-block px-1.5 py-0.5 font-mono text-[8px] font-semibold" style={{ color: '#0A6B3A', backgroundColor: 'rgba(10,107,58,0.1)', border: '1px solid rgba(10,107,58,0.3)' }}>
      {label}
    </span>
  )
}

function FormLabel({ children, required }: { children: React.ReactNode; required?: boolean }) {
  return <label className="font-mono text-[10px] font-semibold text-[#1A1A1A]">{children}{required && <span className="ml-0.5 text-[#CF1322]">*</span>}</label>
}

function FormHint({ children }: { children: React.ReactNode }) {
  return <span className="font-mono text-[9px] font-medium text-[#999999]">{children}</span>
}

function StatusMessage({ type, message }: { type: 'success' | 'error'; message: string }) {
  const color = type === 'success' ? '#0A6B3A' : '#CF1322'
  return (
    <div className="px-6 py-2" style={{ backgroundColor: `${color}08`, borderBottom: '1px solid #E8E8E8' }}>
      <span className="font-mono text-[10px] font-medium" style={{ color }}>{message}</span>
    </div>
  )
}

/* ── 表格列定义 ── */

const tableColumns = ['#', '模型名称', '模型 ID', '提供商', 'API Endpoint', 'API Key', '能力', '状态', '操作']
const colWidths = [40, 'fill', 'fill', 100, 'fill', 120, 100, 60, 80]

function colStyle(w: number | string) {
  return w === 'fill' ? { flex: '1 1 0%', minWidth: 0 } : { width: w, flexShrink: 0 }
}

/* ── 添加/编辑模型弹窗 ── */

interface ModelFormDialogProps {
  open: boolean
  editingModel: ModelConfig | null
  onClose: () => void
  onSaved: () => void
}

function ModelFormDialog({ open, editingModel, onClose, onSaved }: ModelFormDialogProps) {
  const [provider, setProvider] = useState('deepseek')
  const [modelId, setModelId] = useState('')
  const [displayName, setDisplayName] = useState('')
  const [description, setDescription] = useState('')
  const [apiBaseUrl, setApiBaseUrl] = useState('')
  const [apiKey, setApiKey] = useState('')
  const [apiFormat, setApiFormat] = useState<'openai' | 'anthropic'>('openai')
  const [sortOrder, setSortOrder] = useState(0)
  const [capabilities, setCapabilities] = useState('')
  const [providerDropOpen, setProviderDropOpen] = useState(false)
  const [saving, setSaving] = useState(false)
  const [testing, setTesting] = useState(false)
  const [message, setMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null)

  // 初始化表单（创建 vs 编辑）
  useEffect(() => {
    if (!open) return
    setMessage(null)
    if (editingModel) {
      setProvider(editingModel.provider)
      setModelId(editingModel.model_id)
      setDisplayName(editingModel.display_name)
      setDescription(editingModel.description ?? '')
      setApiBaseUrl(editingModel.api_base_url ?? '')
      setApiKey('')
      setSortOrder(editingModel.sort_order)
      setCapabilities(editingModel.capabilities?.join(', ') ?? '')
    } else {
      setProvider('deepseek')
      setModelId('')
      setDisplayName('')
      setDescription('')
      setApiBaseUrl(PROVIDER_LIST[0].baseUrl)
      setApiKey('')
      setApiFormat('openai')
      setSortOrder(0)
      setCapabilities('')
    }
  }, [open, editingModel])

  function handleProviderChange(value: string) {
    setProvider(value)
    setProviderDropOpen(false)
    const p = PROVIDERS[value]
    if (p?.baseUrl) setApiBaseUrl(p.baseUrl)
  }

  async function handleTestConnection() {
    if (!apiKey.trim()) {
      setMessage({ type: 'error', text: '请先填写 API Key' })
      return
    }
    setTesting(true)
    setMessage(null)
    try {
      const resp = await api.post('/model-configs/test-connection', {
        provider,
        api_base_url: apiBaseUrl || null,
        api_key: apiKey.trim(),
        model_id: modelId || null,
      })
      setMessage({ type: 'success', text: resp.data?.message || '连接成功' })
    } catch (err: any) {
      setMessage({ type: 'error', text: err?.response?.data?.details || err?.response?.data?.error || '连接失败' })
    } finally {
      setTesting(false)
    }
  }

  async function handleSave() {
    if (!modelId.trim() || !displayName.trim()) {
      setMessage({ type: 'error', text: '请填写必填字段' })
      return
    }
    if (!editingModel && !apiKey.trim()) {
      setMessage({ type: 'error', text: '请填写 API Key' })
      return
    }
    setSaving(true)
    setMessage(null)
    try {
      const payload: Record<string, unknown> = {
        model_id: modelId.trim(),
        display_name: displayName.trim(),
        description: description.trim() || null,
        provider,
        api_base_url: apiBaseUrl.trim() || null,
        api_format: apiFormat,
        sort_order: sortOrder,
        capabilities: capabilities ? capabilities.split(',').map((s) => s.trim()).filter(Boolean) : [],
      }
      if (apiKey.trim()) payload.api_key = apiKey.trim()

      if (editingModel) {
        await api.put(`/model-configs/${editingModel.id}`, payload)
        setMessage({ type: 'success', text: '模型配置已更新' })
      } else {
        await api.post('/model-configs', payload)
        setMessage({ type: 'success', text: '模型配置已创建' })
      }
      setTimeout(() => { onSaved(); onClose() }, 500)
    } catch (err: any) {
      setMessage({ type: 'error', text: err?.response?.data?.error || '保存失败' })
    } finally {
      setSaving(false)
    }
  }

  const currentProvider = PROVIDERS[provider]
  const isEditing = !!editingModel

  return (
    <Dialog open={open} onOpenChange={(v) => { if (!v) onClose() }}>
      <DialogContent className="sm:max-w-[560px] rounded-none p-0 gap-0 ring-0" style={{ border: '1px solid #E8E8E8' }} showCloseButton={false}>
        {/* 头部 */}
        <div className="flex items-center justify-between px-6 py-5" style={{ borderBottom: '1px solid #E8E8E8' }}>
          <div className="flex items-center gap-2.5">
            <Bot className="h-4 w-4 text-[#0A6B3A]" />
            <span className="text-base font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>
              {isEditing ? '编辑模型配置' : '添加模型配置'}
            </span>
          </div>
          <button onClick={onClose} className="font-mono text-sm font-semibold text-[#999999] hover:text-[#1A1A1A]">✕</button>
        </div>

        {message && <StatusMessage type={message.type} message={message.text} />}

        {/* 表单区 */}
        <div className="flex flex-col gap-4 px-6 py-6" style={{ maxHeight: '60vh', overflowY: 'auto' }}>
          {/* 提供商 */}
          <div className="flex flex-col gap-2">
            <FormLabel required>模型提供商</FormLabel>
            <div className="relative">
              <button onClick={() => setProviderDropOpen(!providerDropOpen)} className="flex w-full items-center justify-between bg-[#F5F5F5] px-3.5 py-2.5" style={{ border: '1px solid #E8E8E8' }}>
                <span className="flex items-center gap-2 font-mono text-[10px] font-medium text-[#1A1A1A]">
                  <span className="text-sm">{currentProvider?.icon}</span><span>{currentProvider?.label}</span>
                </span>
                <ChevronDown className="h-3 w-3 text-[#999999]" />
              </button>
              {providerDropOpen && (
                <div className="absolute z-50 mt-1 w-full border border-[#E8E8E8] bg-white shadow-lg" style={{ maxHeight: 200, overflowY: 'auto' }}>
                  {PROVIDER_LIST.map((p) => (
                    <button key={p.value} onClick={() => handleProviderChange(p.value)} className="flex w-full items-center gap-2.5 px-3.5 py-2.5 hover:bg-[#FAFAFA]" style={{ backgroundColor: provider === p.value ? 'rgba(10,107,58,0.06)' : undefined }}>
                      <span className="text-sm">{p.icon}</span>
                      <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{p.label}</span>
                    </button>
                  ))}
                </div>
              )}
            </div>
          </div>

          {/* 模型 ID */}
          <div className="flex flex-col gap-2">
            <FormLabel required>模型 ID</FormLabel>
            <input value={modelId} onChange={(e) => setModelId(e.target.value)} disabled={isEditing} placeholder="deepseek-chat" className="w-full bg-[#F5F5F5] px-3.5 py-2.5 font-mono text-[10px] font-medium text-[#1A1A1A] outline-none placeholder:text-[#CCCCCC] disabled:opacity-50" style={{ border: '1px solid #E8E8E8' }} />
            <FormHint>模型提供商的标识符，如 deepseek-chat、gpt-4o</FormHint>
          </div>

          {/* 显示名称 */}
          <div className="flex flex-col gap-2">
            <FormLabel required>显示名称</FormLabel>
            <input value={displayName} onChange={(e) => setDisplayName(e.target.value)} placeholder="DeepSeek Chat" className="w-full bg-[#F5F5F5] px-3.5 py-2.5 font-mono text-[10px] font-medium text-[#1A1A1A] outline-none placeholder:text-[#CCCCCC]" style={{ border: '1px solid #E8E8E8' }} />
          </div>

          {/* 描述 */}
          <div className="flex flex-col gap-2">
            <FormLabel>描述</FormLabel>
            <textarea value={description} onChange={(e) => setDescription(e.target.value)} placeholder="模型描述（可选）" rows={2} className="w-full resize-none bg-[#F5F5F5] px-3.5 py-2.5 font-mono text-[10px] font-medium text-[#1A1A1A] outline-none placeholder:text-[#CCCCCC]" style={{ border: '1px solid #E8E8E8' }} />
          </div>

          {/* API Base URL */}
          <div className="flex flex-col gap-2">
            <FormLabel>API Base URL</FormLabel>
            <input value={apiBaseUrl} onChange={(e) => setApiBaseUrl(e.target.value)} placeholder="https://api.deepseek.com/v1" className="w-full bg-[#F5F5F5] px-3.5 py-2.5 font-mono text-[10px] font-medium text-[#1A1A1A] outline-none placeholder:text-[#CCCCCC]" style={{ border: apiBaseUrl ? '1px solid rgba(10,107,58,0.25)' : '1px solid #E8E8E8' }} />
          </div>

          {/* API Key */}
          <div className="flex flex-col gap-2">
            <FormLabel required={!isEditing}>API Key</FormLabel>
            <div className="flex items-center gap-1.5 bg-[#F5F5F5] px-3.5 py-2.5" style={{ border: '1px solid #E8E8E8' }}>
              <EyeOff className="h-3 w-3 shrink-0 text-[#CCCCCC]" />
              <input type="password" value={apiKey} onChange={(e) => setApiKey(e.target.value)} placeholder="sk-..." className="w-full bg-transparent font-mono text-[10px] font-medium text-[#1A1A1A] outline-none placeholder:text-[#CCCCCC]" />
            </div>
            {isEditing && <FormHint>留空则保持原有 Key 不变</FormHint>}
          </div>

          {/* API 格式 */}
          <div className="flex flex-col gap-2">
            <FormLabel>API 格式</FormLabel>
            <div className="flex gap-6">
              {(['openai', 'anthropic'] as const).map((fmt) => (
                <label key={fmt} className="flex cursor-pointer items-center gap-2" onClick={() => setApiFormat(fmt)}>
                  <div className="flex h-4 w-4 items-center justify-center rounded-full" style={{ border: `1.5px solid ${apiFormat === fmt ? '#0A6B3A' : '#D9D9D9'}` }}>
                    {apiFormat === fmt && <div className="h-2 w-2 rounded-full bg-[#0A6B3A]" />}
                  </div>
                  <span className="font-mono text-[10px] font-medium" style={{ color: apiFormat === fmt ? '#1A1A1A' : '#999999' }}>
                    {fmt === 'openai' ? 'OpenAI 兼容' : 'Anthropic 兼容'}
                  </span>
                </label>
              ))}
            </div>
            <FormHint>请选择 API 协议兼容格式</FormHint>
          </div>

          {/* 排序权重 + 能力标签 */}
          <div className="grid grid-cols-2 gap-3">
            <div className="flex flex-col gap-2">
              <FormLabel>排序权重</FormLabel>
              <input type="number" value={sortOrder} onChange={(e) => setSortOrder(Number(e.target.value))} min={0} max={999} className="w-full bg-[#F5F5F5] px-3.5 py-2.5 font-mono text-[10px] font-medium text-[#1A1A1A] outline-none" style={{ border: '1px solid #E8E8E8' }} />
            </div>
            <div className="flex flex-col gap-2">
              <FormLabel>能力标签</FormLabel>
              <input value={capabilities} onChange={(e) => setCapabilities(e.target.value)} placeholder="chat, vision" className="w-full bg-[#F5F5F5] px-3.5 py-2.5 font-mono text-[10px] font-medium text-[#1A1A1A] outline-none placeholder:text-[#CCCCCC]" style={{ border: '1px solid #E8E8E8' }} />
              <FormHint>逗号分隔，如 chat, vision, code</FormHint>
            </div>
          </div>
        </div>

        {/* 底部按钮 */}
        <div className="flex items-center justify-between px-6 py-4" style={{ borderTop: '1px solid #E8E8E8' }}>
          <button onClick={handleTestConnection} disabled={testing} className="flex items-center gap-2 border border-[#E8E8E8] bg-white px-4 py-2.5 font-mono text-[10px] font-semibold text-[#1A1A1A] disabled:opacity-50">
            {testing ? <Loader2 className="h-3 w-3 animate-spin text-[#6a6a6a]" /> : <Plug className="h-3 w-3 text-[#6a6a6a]" />}
            测试连接
          </button>
          <div className="flex gap-3">
            <button onClick={onClose} className="border border-[#E8E8E8] bg-white px-5 py-2.5 font-mono text-[10px] font-semibold text-[#1A1A1A]">取消</button>
            <button onClick={handleSave} disabled={saving} className="flex items-center gap-2 bg-[#0A6B3A] px-5 py-2.5 font-mono text-[10px] font-semibold text-white disabled:opacity-50">
              {saving ? <Loader2 className="h-3 w-3 animate-spin" /> : <Check className="h-3 w-3" />}
              {isEditing ? '保存' : '创建'}
            </button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  )
}

/* ── 主组件 ── */

export default function ModelConfigsPage() {
  const [models, setModels] = useState<ModelConfig[]>([])
  const [loading, setLoading] = useState(false)
  const [searchText, setSearchText] = useState('')
  const [currentPage, setCurrentPage] = useState(1)
  const [modalOpen, setModalOpen] = useState(false)
  const [editingModel, setEditingModel] = useState<ModelConfig | null>(null)

  const loadModels = useCallback(async () => {
    setLoading(true)
    try {
      const resp = await api.get('/model-configs')
      setModels(Array.isArray(resp.data) ? resp.data : [])
    } catch {
      setModels([])
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => { loadModels() }, [loadModels])

  const filteredModels = models.filter((m) => {
    if (!searchText) return true
    const kw = searchText.toLowerCase()
    return m.model_id.toLowerCase().includes(kw) || m.display_name.toLowerCase().includes(kw) || m.provider.toLowerCase().includes(kw)
  })

  const enabledCount = models.filter((m) => m.enabled).length
  const providerCount = new Set(models.map((m) => m.provider)).size

  function handleCreate() {
    setEditingModel(null)
    setModalOpen(true)
  }

  function handleEdit(record: ModelConfig) {
    setEditingModel(record)
    setModalOpen(true)
  }

  async function handleDelete(id: string) {
    if (!confirm('确定删除此模型配置？')) return
    try {
      await api.delete(`/model-configs/${id}`)
      loadModels()
    } catch { /* ignore */ }
  }

  async function handleToggleEnabled(record: ModelConfig) {
    try {
      await api.put(`/model-configs/${record.id}`, { enabled: !record.enabled })
      loadModels()
    } catch { /* ignore */ }
  }

  async function handleSetDefault(record: ModelConfig) {
    try {
      await api.put(`/model-configs/${record.id}`, { is_default: true })
      loadModels()
    } catch { /* ignore */ }
  }

  const statsData = [
    { label: '模型总数', value: String(models.length), icon: Bot,  iconColor: '#1A1A1A', borderColor: '#E8E8E8' },
    { label: '已启用',   value: String(enabledCount),  dot: '#0A6B3A', valueColor: '#0A6B3A', borderColor: 'rgba(10,107,58,0.25)' },
    { label: '已禁用',   value: String(models.length - enabledCount), valueColor: '#CF1322', borderColor: 'rgba(207,19,34,0.25)' },
    { label: '提供商',   value: String(providerCount), icon: Link, iconColor: '#1677FF', borderColor: '#E8E8E8' },
  ]

  return (
    <div className="flex flex-col gap-6">
      {/* 标题区 */}
      <div className="flex items-end justify-between">
        <div className="flex flex-col gap-2">
          <h1 className="text-[22px] font-bold tracking-tight text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px' }}>模型配置</h1>
          <p className="font-mono text-xs text-[#999999]">管理多提供商 AI 模型的接入配置与调度</p>
        </div>
        <div className="flex gap-2">
          <button onClick={loadModels} disabled={loading} className="flex items-center gap-2 border border-[#E8E8E8] bg-white px-4 py-2.5 font-mono text-[9px] font-semibold text-[#1A1A1A] disabled:opacity-50">
            <RefreshCw className={`h-3 w-3 text-[#6a6a6a] ${loading ? 'animate-spin' : ''}`} />
            刷新
          </button>
          <button onClick={handleCreate} className="flex items-center gap-2 bg-[#0A6B3A] px-4 py-2.5 font-mono text-[9px] font-semibold text-white">
            <Plus className="h-3 w-3" />
            添加模型
          </button>
        </div>
      </div>

      {/* 统计卡片 */}
      <div className="grid grid-cols-4 gap-3">
        {statsData.map((s) => (
          <div key={s.label} className="flex flex-col gap-3 bg-white p-4" style={{ border: `1px solid ${s.borderColor}` }}>
            <span className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{s.label}</span>
            <div className="flex items-center gap-2">
              {s.dot && <div className="h-2 w-2 rounded-full" style={{ backgroundColor: s.dot }} />}
              {s.icon && <s.icon className="h-3.5 w-3.5" style={{ color: s.iconColor }} />}
              <span className="text-[28px] font-bold tracking-tight" style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px', color: s.valueColor ?? '#1A1A1A' }}>{s.value}</span>
            </div>
          </div>
        ))}
      </div>

      {/* 筛选栏 */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="flex w-[280px] items-center gap-2.5 border border-[#E8E8E8] bg-white px-3.5 py-2">
            <Search className="h-3 w-3 text-[#6a6a6a]" />
            <input value={searchText} onChange={(e) => { setSearchText(e.target.value); setCurrentPage(1) }} placeholder="搜索模型名称、ID 或提供商..." className="w-full bg-transparent font-mono text-[10px] font-medium text-[#1A1A1A] outline-none placeholder:text-[#CCCCCC]" />
          </div>
          <div className="flex items-center gap-2 border border-[#E8E8E8] bg-white px-3.5 py-2">
            <span className="font-mono text-[10px] font-medium text-[#999999]">提供商</span>
            <ChevronDown className="h-3 w-3 text-[#999999]" />
          </div>
          <div className="flex items-center gap-2 border border-[#E8E8E8] bg-white px-3.5 py-2">
            <span className="font-mono text-[10px] font-medium text-[#999999]">状态</span>
            <ChevronDown className="h-3 w-3 text-[#999999]" />
          </div>
        </div>
        <span className="font-mono text-[10px] font-medium text-[#999999]">共 {models.length} 个模型配置</span>
      </div>

      {/* 表格 */}
      <div className="bg-white" style={{ border: '1px solid #E8E8E8' }}>
        <div className="flex items-center px-4 py-2.5" style={{ borderBottom: '1px solid #E8E8E8', backgroundColor: '#FAFAFA' }}>
          {tableColumns.map((h, i) => (
            <span key={h} className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]" style={colStyle(colWidths[i])}>{h}</span>
          ))}
        </div>
        {filteredModels.length === 0 ? (
          <div className="flex flex-col items-center justify-center gap-3 py-16">
            <Bot className="h-10 w-10 text-[#CCCCCC]" />
            <span className="font-mono text-[10px] font-medium text-[#999999]">
              {loading ? '加载中...' : '暂无模型配置，点击「添加模型」创建'}
            </span>
          </div>
        ) : (
          filteredModels.map((m, i) => (
            <div key={m.id} className="flex items-center px-4 py-3" style={{ borderBottom: i < filteredModels.length - 1 ? '1px solid #E8E8E8' : 'none' }}>
              <span className="font-mono text-[10px] font-medium text-[#999999]" style={colStyle(40)}>{m.sort_order}</span>
              <div className="flex items-center gap-1.5" style={colStyle('fill')}>
                <span className="font-mono text-[10px] font-semibold text-[#1A1A1A]">{m.display_name}</span>
                {m.is_default && <span className="inline-flex items-center gap-0.5 px-1.5 py-0.5 font-mono text-[8px] font-semibold" style={{ color: '#D48700', backgroundColor: '#D4870015', border: '1px solid #D48700' }}>⭐ 默认</span>}
              </div>
              <span className="font-mono text-[10px] font-medium text-[#999999]" style={colStyle('fill')}>{m.model_id}</span>
              <div style={colStyle(100)}><ProviderTag provider={m.provider} /></div>
              <span className="truncate font-mono text-[9px] font-medium text-[#999999]" style={colStyle('fill')}>{m.api_base_url ?? '—'}</span>
              <div className="flex items-center gap-1" style={colStyle(120)}>
                {m.api_key ? (<><EyeOff className="h-2.5 w-2.5 text-[#CCCCCC]" /><span className="font-mono text-[9px] font-medium text-[#999999]">{maskApiKey(m.api_key)}</span></>) : (<span className="font-mono text-[10px] font-medium text-[#CCCCCC]">—</span>)}
              </div>
              <div className="flex flex-wrap gap-1" style={colStyle(100)}>
                {m.capabilities?.length ? m.capabilities.map((c) => <CapabilityTag key={c} label={c} />) : <span className="font-mono text-[10px] text-[#CCCCCC]">—</span>}
              </div>
              <div style={colStyle(60)}><ToggleSwitch on={m.enabled} onChange={() => handleToggleEnabled(m)} /></div>
              <div className="flex items-center gap-2" style={colStyle(80)}>
                {!m.is_default && <button onClick={() => handleSetDefault(m)} className="text-[#6a6a6a] hover:text-[#D48700]" title="设为默认"><Star className="h-3.5 w-3.5" /></button>}
                <button onClick={() => handleEdit(m)} className="text-[#6a6a6a] hover:text-[#0A6B3A]" title="编辑"><Pencil className="h-3.5 w-3.5" /></button>
                <button onClick={() => handleDelete(m.id)} className="text-[#6a6a6a] hover:text-[#CF1322]" title="删除"><Trash2 className="h-3.5 w-3.5" /></button>
              </div>
            </div>
          ))
        )}
        <TablePagination current={currentPage} total={filteredModels.length} pageSize={10} onChange={setCurrentPage} />
      </div>

      <ModelFormDialog open={modalOpen} editingModel={editingModel} onClose={() => { setModalOpen(false); setEditingModel(null) }} onSaved={loadModels} />
    </div>
  )
}
