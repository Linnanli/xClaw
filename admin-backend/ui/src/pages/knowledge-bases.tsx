import { Plus, Upload, Search, Trash2, Loader2, X, ToggleLeft, ToggleRight } from 'lucide-react'
import { useState, useEffect, useCallback } from 'react'
import { api } from '@/lib/api'
import { TablePagination } from '@/components/ui/table-pagination'
import { StatusTag } from '@/components/ui/status-tag'
import { Dialog, DialogContent } from '@/components/ui/dialog'
import type { KnowledgeBase, KBDocument } from '@/types'

// ── 工具函数 ──────────────────────────────────────────────────

function formatFileSize(bytes: number): string {
  if (bytes === 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB']
  const i = Math.floor(Math.log(bytes) / Math.log(1024))
  return `${(bytes / Math.pow(1024, i)).toFixed(1)} ${units[i]}`
}

function formatAccessLabel(kb: KnowledgeBase): string {
  const depts = kb.allowed_departments?.length ?? 0
  const roles = kb.allowed_roles?.length ?? 0
  if (depts === 0 && roles === 0) return '全员可访问'
  const parts: string[] = []
  if (depts > 0) parts.push(`${depts} 个部门`)
  if (roles > 0) parts.push(`${roles} 个角色`)
  return parts.join('、') + '可访问'
}

// ── 常量映射 ──────────────────────────────────────────────────

const fileTypeMap: Record<string, { label: string; color: string }> = {
  pdf:  { label: 'PDF',  color: '#CF1322' },
  docx: { label: 'DOCX', color: '#1677FF' },
  md:   { label: 'MD',   color: '#0A6B3A' },
  txt:  { label: 'TXT',  color: '#999999' },
}

const docStatusMap: Record<string, { label: string; color: string }> = {
  pending:    { label: '待处理', color: '#999999' },
  processing: { label: '处理中', color: '#D48700' },
  completed:  { label: '已完成', color: '#0A6B3A' },
  failed:     { label: '失败',   color: '#CF1322' },
}

// ── 主页面 ────────────────────────────────────────────────────

export default function KnowledgeBasesPage() {
  const [kbs, setKbs] = useState<KnowledgeBase[]>([])
  const [_total, setTotal] = useState(0)
  const [loading, setLoading] = useState(true)
  const [search, setSearch] = useState('')
  const [error, setError] = useState<string | null>(null)

  const [selectedKb, setSelectedKb] = useState<KnowledgeBase | null>(null)
  const [docs, setDocs] = useState<KBDocument[]>([])
  const [docsTotal, setDocsTotal] = useState(0)
  const [docsPage, setDocsPage] = useState(1)
  const [docsLoading, setDocsLoading] = useState(false)
  const docsPageSize = 10

  const [createOpen, setCreateOpen] = useState(false)
  const [uploadOpen, setUploadOpen] = useState(false)
  const [searchOpen, setSearchOpen] = useState(false)
  const [targetKb, setTargetKb] = useState<KnowledgeBase | null>(null)

  const fetchKbs = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const params = new URLSearchParams()
      if (search.trim()) params.set('search', search.trim())
      const { data } = await api.get(`/knowledge-bases?${params}`)
      setKbs(data.data ?? [])
      setTotal(data.total ?? 0)
    } catch (e) {
      setError('加载知识库失败，请刷新重试')
      console.error('[KnowledgeBases] fetchKbs error:', e)
    }
    setLoading(false)
  }, [search])

  useEffect(() => { fetchKbs() }, [fetchKbs])

  const fetchDocs = useCallback(async (kb: KnowledgeBase, page: number) => {
    setDocsLoading(true)
    try {
      const params = new URLSearchParams({ page: String(page), page_size: String(docsPageSize) })
      const { data } = await api.get(`/knowledge-bases/${kb.id}/documents?${params}`)
      setDocs(data.data ?? [])
      setDocsTotal(data.total ?? 0)
    } catch (e) {
      console.error('[KnowledgeBases] fetchDocs error:', e)
    }
    setDocsLoading(false)
  }, [])

  const selectKb = (kb: KnowledgeBase) => {
    setSelectedKb(kb)
    setDocsPage(1)
    fetchDocs(kb, 1)
  }

  const handleDocsPageChange = (page: number) => {
    setDocsPage(page)
    if (selectedKb) fetchDocs(selectedKb, page)
  }

  const toggleEnabled = async (kb: KnowledgeBase) => {
    try {
      await api.put(`/knowledge-bases/${kb.id}`, { enabled: !kb.enabled })
      fetchKbs()
      if (selectedKb?.id === kb.id) setSelectedKb({ ...selectedKb, enabled: !kb.enabled })
    } catch (e) {
      console.error('[KnowledgeBases] toggleEnabled error:', e)
      alert('操作失败，请重试')
    }
  }

  const deleteKb = async (kb: KnowledgeBase) => {
    if (!confirm(`确认删除知识库「${kb.name}」？此操作不可撤销。`)) return
    try {
      await api.delete(`/knowledge-bases/${kb.id}`)
      if (selectedKb?.id === kb.id) { setSelectedKb(null); setDocs([]) }
      fetchKbs()
    } catch (e) {
      console.error('[KnowledgeBases] deleteKb error:', e)
      alert('删除失败，请重试')
    }
  }

  const deleteDoc = async (doc: KBDocument) => {
    if (!selectedKb) return
    try {
      await api.delete(`/knowledge-bases/${selectedKb.id}/documents/${doc.id}`)
      fetchDocs(selectedKb, docsPage)
      fetchKbs()
    } catch (e) {
      console.error('[KnowledgeBases] deleteDoc error:', e)
      alert('删除文档失败，请重试')
    }
  }

  return (
    <div className="flex flex-col gap-6">
      {/* 标题区 */}
      <div className="flex items-end justify-between">
        <div className="flex flex-col gap-2">
          <h1 className="text-[22px] font-bold tracking-tight text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px' }}>
            知识库管理
          </h1>
          <p className="font-mono text-xs text-[#999999]">管理企业私有知识库，增强 AI 助手回答能力</p>
        </div>
        <div className="flex items-center gap-3">
          <div className="flex w-[200px] items-center gap-2.5 border border-[#E8E8E8] bg-white px-3.5 py-2">
            <Search className="h-3 w-3 text-[#6a6a6a]" />
            <input
              className="w-full bg-transparent font-mono text-[10px] font-medium text-[#1A1A1A] placeholder-[#CCCCCC] outline-none"
              placeholder="搜索知识库..."
              value={search}
              onChange={(e) => setSearch(e.target.value)}
            />
          </div>
          <button
            className="flex items-center gap-2 bg-[#0A6B3A] px-4 py-2.5 font-mono text-[9px] font-semibold text-white"
            onClick={() => setCreateOpen(true)}
          >
            <Plus className="h-3 w-3" />
            新建知识库
          </button>
        </div>
      </div>

      {/* 错误提示 */}
      {error && (
        <div className="border border-[#FFCCC7] bg-[#FFF2F0] px-4 py-3 font-mono text-[10px] text-[#CF1322]">{error}</div>
      )}

      {/* 知识库卡片 */}
      {loading ? (
        <div className="flex items-center justify-center py-12"><Loader2 className="h-5 w-5 animate-spin text-[#999]" /></div>
      ) : kbs.length === 0 ? (
        <div className="flex items-center justify-center py-12"><span className="font-mono text-[10px] text-[#999]">暂无知识库</span></div>
      ) : (
        <div className="grid grid-cols-3 gap-4">
          {kbs.map((kb) => (
            <div
              key={kb.id}
              className={`flex cursor-pointer flex-col gap-4 bg-white p-5 transition-shadow hover:shadow-sm ${selectedKb?.id === kb.id ? 'ring-1 ring-[#0A6B3A]' : ''}`}
              style={{ border: `1px solid ${selectedKb?.id === kb.id ? '#0A6B3A' : '#E8E8E8'}` }}
              onClick={() => selectKb(kb)}
            >
              <div className="flex items-center justify-between">
                <span className="text-sm font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>
                  {kb.name}
                </span>
                <StatusTag label={kb.enabled ? '已启用' : '已禁用'} color={kb.enabled ? '#0A6B3A' : '#999999'} />
              </div>
              <span className="font-mono text-[10px] font-medium text-[#999999]">{kb.description || '—'}</span>
              <div className="flex gap-4">
                <div className="flex flex-col gap-1">
                  <span className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">文档</span>
                  <span className="font-mono text-[14px] font-bold text-[#1A1A1A]">{kb.document_count}</span>
                </div>
              </div>
              {/* 访问权限：从 allowed_departments/allowed_roles 派生 */}
              <span className="font-mono text-[10px] font-medium text-[#999999]">{formatAccessLabel(kb)}</span>
              <div className="flex items-center gap-2" onClick={(e) => e.stopPropagation()}>
                <button
                  className="flex items-center gap-1.5 border border-[#E8E8E8] bg-white px-3 py-1.5 font-mono text-[9px] font-semibold text-[#1A1A1A] hover:bg-[#FAFAFA]"
                  onClick={() => { setTargetKb(kb); setUploadOpen(true) }}
                >
                  <Upload className="h-3 w-3 text-[#6a6a6a]" />
                  上传文档
                </button>
                <button
                  className="flex items-center gap-1.5 border border-[#E8E8E8] bg-white px-3 py-1.5 font-mono text-[9px] font-semibold text-[#1A1A1A] hover:bg-[#FAFAFA]"
                  onClick={() => { setTargetKb(kb); setSearchOpen(true) }}
                >
                  <Search className="h-3 w-3 text-[#6a6a6a]" />
                  检索测试
                </button>
                <button
                  className="ml-auto p-1.5 text-[#999] hover:text-[#0A6B3A]"
                  title={kb.enabled ? '禁用' : '启用'}
                  onClick={() => toggleEnabled(kb)}
                >
                  {kb.enabled
                    ? <ToggleRight className="h-4 w-4 text-[#0A6B3A]" />
                    : <ToggleLeft className="h-4 w-4 text-[#999]" />}
                </button>
                <button
                  className="p-1.5 text-[#999] hover:text-[#CF1322]"
                  title="删除"
                  onClick={() => deleteKb(kb)}
                >
                  <Trash2 className="h-3.5 w-3.5" />
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* 文档列表区 */}
      {selectedKb && (
        <>
          <div className="flex items-center justify-between">
            <span className="text-base font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>
              {selectedKb.name} — 文档管理
            </span>
            <span className="font-mono text-[10px] font-medium text-[#999999]">{docsTotal} 个文档</span>
          </div>

          <div className="bg-white" style={{ border: '1px solid #E8E8E8' }}>
            <div className="grid grid-cols-7 px-4 py-2.5" style={{ borderBottom: '1px solid #E8E8E8' }}>
              {['文件名', '类型', '大小', '切片', '状态', '上传时间', '操作'].map((h) => (
                <span key={h} className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{h}</span>
              ))}
            </div>

            {docsLoading ? (
              <div className="flex items-center justify-center py-12"><Loader2 className="h-5 w-5 animate-spin text-[#999]" /></div>
            ) : docs.length === 0 ? (
              <div className="flex items-center justify-center py-12"><span className="font-mono text-[10px] text-[#999]">暂无文档</span></div>
            ) : (
              docs.map((d, i) => {
                const ft = fileTypeMap[d.file_type.toLowerCase()] ?? { label: d.file_type.toUpperCase(), color: '#999999' }
                const st = docStatusMap[d.status] ?? { label: d.status, color: '#999999' }
                const date = d.uploaded_at ? d.uploaded_at.slice(0, 10) : '—'
                return (
                  <div
                    key={d.id}
                    className="grid grid-cols-7 items-center px-4 py-3"
                    style={{ borderBottom: i < docs.length - 1 ? '1px solid #E8E8E8' : 'none' }}
                  >
                    <span className="truncate font-mono text-[10px] font-medium text-[#1A1A1A]">{d.filename}</span>
                    <div><StatusTag label={ft.label} color={ft.color} /></div>
                    {/* file_size 是字节数，用 formatFileSize 转换 */}
                    <span className="font-mono text-[10px] font-medium text-[#999999]">{formatFileSize(d.file_size)}</span>
                    <span className="font-mono text-[10px] font-medium text-[#999999]">{d.chunk_count}</span>
                    <div><StatusTag label={st.label} color={st.color} /></div>
                    <span className="font-mono text-[10px] font-medium text-[#999999]">{date}</span>
                    <button
                      className="w-fit p-1 text-[#999] hover:text-[#CF1322]"
                      onClick={() => deleteDoc(d)}
                    >
                      <Trash2 className="h-3.5 w-3.5" />
                    </button>
                  </div>
                )
              })
            )}
            <TablePagination current={docsPage} total={docsTotal} pageSize={docsPageSize} onChange={handleDocsPageChange} />
          </div>
        </>
      )}

      <CreateKbDialog
        open={createOpen}
        onClose={() => setCreateOpen(false)}
        onCreated={() => { setCreateOpen(false); fetchKbs() }}
      />

      {targetKb && (
        <UploadDocDialog
          open={uploadOpen}
          kb={targetKb}
          onClose={() => setUploadOpen(false)}
          onUploaded={() => {
            setUploadOpen(false)
            if (selectedKb?.id === targetKb.id) fetchDocs(targetKb, docsPage)
            fetchKbs()
          }}
        />
      )}

      {targetKb && (
        <SearchDialog
          open={searchOpen}
          kb={targetKb}
          onClose={() => setSearchOpen(false)}
        />
      )}
    </div>
  )
}

// ── 新建知识库弹窗 ────────────────────────────────────────────

function CreateKbDialog({ open, onClose, onCreated }: {
  open: boolean; onClose: () => void; onCreated: () => void
}) {
  const [name, setName] = useState('')
  const [description, setDescription] = useState('')
  const [submitting, setSubmitting] = useState(false)

  const handleSubmit = async () => {
    if (!name.trim()) return
    setSubmitting(true)
    try {
      await api.post('/knowledge-bases', { name: name.trim(), description: description.trim() || undefined })
      setName(''); setDescription('')
      onCreated()
    } catch (e) {
      console.error('[CreateKbDialog] error:', e)
      alert('创建失败，请重试')
    }
    setSubmitting(false)
  }

  return (
    <Dialog open={open} onOpenChange={(v) => !v && onClose()}>
      <DialogContent className="max-w-[440px] p-0">
        <div className="flex items-center justify-between border-b border-[#E8E8E8] px-6 py-4">
          <span className="font-mono text-xs font-bold text-[#1A1A1A]">新建知识库</span>
          <button onClick={onClose}><X className="h-4 w-4 text-[#999]" /></button>
        </div>
        <div className="flex flex-col gap-4 px-6 py-5">
          <div className="flex flex-col gap-1.5">
            <label className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">名称 *</label>
            <input
              className="border border-[#E8E8E8] bg-white px-3 py-2 font-mono text-[10px] text-[#1A1A1A] outline-none focus:border-[#0A6B3A]"
              placeholder="输入知识库名称"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          </div>
          <div className="flex flex-col gap-1.5">
            <label className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">描述</label>
            <textarea
              className="border border-[#E8E8E8] bg-white px-3 py-2 font-mono text-[10px] text-[#1A1A1A] outline-none focus:border-[#0A6B3A]"
              placeholder="输入描述（可选）"
              rows={3}
              value={description}
              onChange={(e) => setDescription(e.target.value)}
            />
          </div>
          <div className="flex justify-end gap-2 pt-1">
            <button className="border border-[#E8E8E8] px-4 py-2 font-mono text-[9px] font-semibold text-[#999]" onClick={onClose}>取消</button>
            <button
              className="flex items-center gap-2 bg-[#0A6B3A] px-4 py-2 font-mono text-[9px] font-semibold text-white disabled:opacity-50"
              disabled={!name.trim() || submitting}
              onClick={handleSubmit}
            >
              {submitting && <Loader2 className="h-3 w-3 animate-spin" />}
              创建
            </button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  )
}

// ── 上传文档弹窗 ──────────────────────────────────────────────

function UploadDocDialog({ open, kb, onClose, onUploaded }: {
  open: boolean; kb: KnowledgeBase; onClose: () => void; onUploaded: () => void
}) {
  const [filename, setFilename] = useState('')
  const [fileType, setFileType] = useState('pdf')
  const [fileSizeBytes, setFileSizeBytes] = useState('')
  const [submitting, setSubmitting] = useState(false)

  const handleSubmit = async () => {
    const sizeNum = parseInt(fileSizeBytes, 10)
    if (!filename.trim() || isNaN(sizeNum) || sizeNum < 0) return
    setSubmitting(true)
    try {
      await api.post(`/knowledge-bases/${kb.id}/documents`, {
        filename: filename.trim(),
        file_type: fileType,
        file_size: sizeNum,
      })
      setFilename(''); setFileType('pdf'); setFileSizeBytes('')
      onUploaded()
    } catch (e) {
      console.error('[UploadDocDialog] error:', e)
      alert('上传失败，请重试')
    }
    setSubmitting(false)
  }

  return (
    <Dialog open={open} onOpenChange={(v) => !v && onClose()}>
      <DialogContent className="max-w-[440px] p-0">
        <div className="flex items-center justify-between border-b border-[#E8E8E8] px-6 py-4">
          <span className="font-mono text-xs font-bold text-[#1A1A1A]">上传文档 — {kb.name}</span>
          <button onClick={onClose}><X className="h-4 w-4 text-[#999]" /></button>
        </div>
        <div className="flex flex-col gap-4 px-6 py-5">
          <div className="flex flex-col gap-1.5">
            <label className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">文件名 *</label>
            <input
              className="border border-[#E8E8E8] bg-white px-3 py-2 font-mono text-[10px] text-[#1A1A1A] outline-none focus:border-[#0A6B3A]"
              placeholder="例：产品需求文档.pdf"
              value={filename}
              onChange={(e) => setFilename(e.target.value)}
            />
          </div>
          <div className="flex gap-3">
            <div className="flex flex-1 flex-col gap-1.5">
              <label className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">类型</label>
              <select
                className="border border-[#E8E8E8] bg-white px-3 py-2 font-mono text-[10px] text-[#1A1A1A] outline-none"
                value={fileType}
                onChange={(e) => setFileType(e.target.value)}
              >
                <option value="pdf">PDF</option>
                <option value="docx">DOCX</option>
                <option value="md">MD</option>
                <option value="txt">TXT</option>
              </select>
            </div>
            <div className="flex flex-1 flex-col gap-1.5">
              <label className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">大小（字节）*</label>
              <input
                className="border border-[#E8E8E8] bg-white px-3 py-2 font-mono text-[10px] text-[#1A1A1A] outline-none focus:border-[#0A6B3A]"
                placeholder="例：2457600"
                type="number"
                min="0"
                value={fileSizeBytes}
                onChange={(e) => setFileSizeBytes(e.target.value)}
              />
            </div>
          </div>
          <div className="flex justify-end gap-2 pt-1">
            <button className="border border-[#E8E8E8] px-4 py-2 font-mono text-[9px] font-semibold text-[#999]" onClick={onClose}>取消</button>
            <button
              className="flex items-center gap-2 bg-[#0A6B3A] px-4 py-2 font-mono text-[9px] font-semibold text-white disabled:opacity-50"
              disabled={!filename.trim() || !fileSizeBytes || submitting}
              onClick={handleSubmit}
            >
              {submitting && <Loader2 className="h-3 w-3 animate-spin" />}
              上传
            </button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  )
}

// ── 检索测试弹窗 ──────────────────────────────────────────────

function SearchDialog({ open, kb, onClose }: {
  open: boolean; kb: KnowledgeBase; onClose: () => void
}) {
  const [query, setQuery] = useState('')
  const [results, setResults] = useState<unknown[]>([])
  const [message, setMessage] = useState('')
  const [searching, setSearching] = useState(false)
  const [searched, setSearched] = useState(false)

  const handleSearch = async () => {
    if (!query.trim()) return
    setSearching(true)
    setSearched(false)
    try {
      const { data } = await api.post(`/knowledge-bases/${kb.id}/search`, { query: query.trim() })
      setResults(Array.isArray(data.results) ? data.results : [])
      setMessage(data.message || '')
    } catch (e) {
      console.error('[SearchDialog] error:', e)
      setResults([])
      setMessage('检索失败，请重试')
    }
    setSearching(false)
    setSearched(true)
  }

  const handleClose = () => {
    setQuery(''); setResults([]); setMessage(''); setSearched(false)
    onClose()
  }

  return (
    <Dialog open={open} onOpenChange={(v) => !v && handleClose()}>
      <DialogContent className="max-w-[520px] p-0">
        <div className="flex items-center justify-between border-b border-[#E8E8E8] px-6 py-4">
          <span className="font-mono text-xs font-bold text-[#1A1A1A]">检索测试 — {kb.name}</span>
          <button onClick={handleClose}><X className="h-4 w-4 text-[#999]" /></button>
        </div>
        <div className="flex flex-col gap-4 px-6 py-5">
          <div className="flex gap-2">
            <input
              className="flex-1 border border-[#E8E8E8] bg-white px-3 py-2 font-mono text-[10px] text-[#1A1A1A] outline-none focus:border-[#0A6B3A]"
              placeholder="输入检索词..."
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && handleSearch()}
            />
            <button
              className="flex items-center gap-2 bg-[#0A6B3A] px-4 py-2 font-mono text-[9px] font-semibold text-white disabled:opacity-50"
              disabled={!query.trim() || searching}
              onClick={handleSearch}
            >
              {searching ? <Loader2 className="h-3 w-3 animate-spin" /> : <Search className="h-3 w-3" />}
              检索
            </button>
          </div>

          {searched && (
            <div className="flex flex-col gap-2">
              {message && <p className="font-mono text-[10px] text-[#999]">{message}</p>}
              {results.length === 0 ? (
                <p className="font-mono text-[10px] text-[#999]">未找到相关结果</p>
              ) : (
                results.map((r, i) => (
                  <div key={i} className="rounded border border-[#E8E8E8] bg-[#FAFAFA] px-3 py-2">
                    <pre className="whitespace-pre-wrap font-mono text-[10px] text-[#333]">{JSON.stringify(r, null, 2)}</pre>
                  </div>
                ))
              )}
            </div>
          )}
        </div>
      </DialogContent>
    </Dialog>
  )
}
