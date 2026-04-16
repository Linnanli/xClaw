import { Search, Pencil, Trash2, ChevronDown, Loader2, Shield, BookOpen, AlertTriangle, History, ShieldOff, Upload, Plus, Check } from 'lucide-react'
import { useState, useEffect, useCallback, useRef } from 'react'
import { TablePagination } from '@/components/ui/table-pagination'
import { StatusTag } from '@/components/ui/status-tag'
import { ToggleSwitch } from '@/components/ui/toggle-switch'
import { Dialog, DialogContent } from '@/components/ui/dialog'
import { api } from '@/lib/api'

/* ── 类型定义 ── */

interface DlpRule {
  id: string; name: string; pattern: string; replacement?: string
  severity: 'low' | 'medium' | 'high' | 'critical'
  description?: string; enabled: boolean; category: string
  classification_level?: string
  rule_type: 'regex' | 'keyword' | 'dictionary'
  updated_at: string
}

interface DlpDictionary {
  id: string; name: string; description?: string
  keyword_count: number; keywords: string[]
  updated_at: string
}

interface SensitiveOperation {
  id: string; name: string; operation_type: string
  risk_level: string; requires_approval: boolean
  description?: string; enabled: boolean; updated_at: string
}

interface PolicyChangeRecord {
  id: string; rule_id: string; rule_type: string; rule_name?: string
  change_type: string; field_changed?: string
  old_value?: any; new_value?: any
  changed_by_name?: string; changed_at: string
}

/* ── 常量映射 ── */

const SEVERITY_MAP: Record<string, { label: string; color: string }> = {
  low: { label: '低危', color: '#0A6B3A' },
  medium: { label: '中危', color: '#D4B200' },
  high: { label: '高危', color: '#D48700' },
  critical: { label: '严重', color: '#CF1322' },
}

const CATEGORY_MAP: Record<string, string> = {
  pii: '个人身份信息', financial: '金融信息', health: '健康医疗',
  credential: '凭证信息', confidential: '机密信息', other: '其他',
}

const RISK_LEVEL_MAP: Record<string, { label: string; color: string }> = {
  low: { label: '低', color: '#0A6B3A' },
  medium: { label: '中', color: '#D48700' },
  high: { label: '高', color: '#CF1322' },
  critical: { label: '严重', color: '#CF1322' },
}

const CHANGE_TYPE_MAP: Record<string, { label: string; color: string }> = {
  create: { label: '创建', color: '#0A6B3A' },
  update: { label: '修改', color: '#1677FF' },
  delete: { label: '删除', color: '#CF1322' },
  enable: { label: '启用', color: '#0A6B3A' },
  disable: { label: '禁用', color: '#D48700' },
}

const RULE_TYPE_MAP: Record<string, { label: string; color: string }> = {
  regex: { label: '正则', color: '#1677FF' },
  keyword: { label: '关键字', color: '#D48700' },
  dictionary: { label: '字典', color: '#0A6B3A' },
}

const SEVERITY_OPTIONS = Object.entries(SEVERITY_MAP).map(([k, v]) => ({ value: k, label: v.label }))
const CATEGORY_OPTIONS = Object.entries(CATEGORY_MAP).map(([k, v]) => ({ value: k, label: v }))

/* ── 创建 DLP 规则弹窗 ── */

/* ── DLP 规则弹窗（创建/编辑） ── */

interface DlpRuleFormDialogProps {
  open: boolean
  editingRule: DlpRule | null
  onClose: () => void
  onSaved: () => void
}

const MATCH_MODE_OPTIONS = [
  { value: 'contains', label: '包含匹配' },
  { value: 'exact', label: '精确匹配' },
  { value: 'whole_word', label: '全词匹配' },
]

function DlpRuleFormDialog({ open, editingRule, onClose, onSaved }: DlpRuleFormDialogProps) {
  const isEditing = !!editingRule
  const [name, setName] = useState('')
  const [ruleType, setRuleType] = useState<'regex' | 'keyword' | 'dictionary'>('regex')
  const [pattern, setPattern] = useState('')
  const [replacement, setReplacement] = useState('')
  const [severity, setSeverity] = useState('medium')
  const [category, setCategory] = useState('pii')
  const [description, setDescription] = useState('')
  const [matchMode, setMatchMode] = useState('contains')
  const [caseSensitive, setCaseSensitive] = useState(false)
  const [keywords, setKeywords] = useState<string[]>([])
  const [keywordInput, setKeywordInput] = useState('')
  const [dictionaries, setDictionaries] = useState<DlpDictionary[]>([])
  const [selectedDictId, setSelectedDictId] = useState('')
  const [testText, setTestText] = useState('')
  const [testResult, setTestResult] = useState<{ matched: boolean; matches: string[]; replaced: string } | null>(null)
  const [showTest, setShowTest] = useState(false)
  const [regexError, setRegexError] = useState<string | null>(null)
  const [saving, setSaving] = useState(false)
  const [msg, setMsg] = useState<{ type: 'success' | 'error'; text: string } | null>(null)
  const [classificationLevel, setClassificationLevel] = useState('')

  useEffect(() => {
    if (!open) return
    setMsg(null); setTestResult(null); setTestText(''); setShowTest(false); setRegexError(null)
    if (editingRule) {
      setName(editingRule.name); setRuleType(editingRule.rule_type || 'regex')
      setPattern(editingRule.pattern); setReplacement(editingRule.replacement ?? '')
      setSeverity(editingRule.severity); setCategory(editingRule.category)
      setDescription(editingRule.description ?? '')
      setClassificationLevel(editingRule.classification_level ?? '')
    } else {
      setName(''); setRuleType('regex'); setPattern(''); setReplacement('')
      setSeverity('medium'); setCategory('pii'); setDescription('')
      setKeywords([]); setKeywordInput(''); setSelectedDictId(''); setMatchMode('contains'); setCaseSensitive(false)
      setClassificationLevel('')
    }
  }, [open, editingRule])

  useEffect(() => {
    if (open && ruleType === 'dictionary') {
      api.get('/dlp-dictionaries').then((r) => setDictionaries(r.data.dictionaries ?? [])).catch((err) => console.error('加载字典失败', err))
    }
  }, [open, ruleType])

  function handlePatternChange(val: string) {
    setPattern(val); setTestResult(null)
    if (val && ruleType === 'regex') {
      try { new RegExp(val); setRegexError(null) } catch (e: any) { setRegexError(e.message) }
    } else { setRegexError(null) }
  }

  function addKeyword() {
    const kw = keywordInput.trim()
    if (!kw || keywords.includes(kw)) return
    setKeywords([...keywords, kw]); setKeywordInput(''); setTestResult(null)
  }

  function handleTest() {
    if (!testText) return
    const rep = replacement || '***'
    if (ruleType === 'regex') {
      if (!pattern) return
      try {
        const re = new RegExp(pattern, 'g')
        const matches = testText.match(re) || []
        setTestResult({ matched: matches.length > 0, matches, replaced: testText.replace(re, rep) })
      } catch { setTestResult(null) }
    } else {
      const kws = ruleType === 'keyword' ? keywords : (dictionaries.find((d) => d.id === selectedDictId)?.keywords ?? [])
      if (!kws.length) return
      const allMatches: string[] = []; let result = testText
      for (const kw of kws) {
        const search = caseSensitive ? testText : testText.toLowerCase()
        const target = caseSensitive ? kw : kw.toLowerCase()
        let idx = search.indexOf(target)
        while (idx !== -1) { allMatches.push(testText.substring(idx, idx + kw.length)); idx = search.indexOf(target, idx + 1) }
      }
      for (const m of allMatches) result = result.split(m).join(rep)
      setTestResult({ matched: allMatches.length > 0, matches: allMatches, replaced: result })
    }
  }

  async function handleSave() {
    if (!name.trim()) { setMsg({ type: 'error', text: '请输入规则名称' }); return }
    if (ruleType === 'regex' && !pattern.trim()) { setMsg({ type: 'error', text: '请输入匹配模式' }); return }
    if (ruleType === 'regex' && regexError) { setMsg({ type: 'error', text: '正则表达式无效' }); return }
    if (ruleType === 'keyword' && !keywords.length) { setMsg({ type: 'error', text: '请至少添加一个关键字' }); return }
    if (ruleType === 'dictionary' && !selectedDictId) { setMsg({ type: 'error', text: '请选择一个字典' }); return }
    setSaving(true); setMsg(null)
    try {
      let finalPattern = pattern.trim()
      let ruleConfig: Record<string, unknown> | null = null
      if (ruleType === 'keyword') {
        finalPattern = keywords.join(',')
        ruleConfig = { keywords, match_mode: matchMode, case_sensitive: caseSensitive }
      } else if (ruleType === 'dictionary') {
        const dict = dictionaries.find((d) => d.id === selectedDictId)
        finalPattern = dict?.keywords.join(',') ?? ''
        ruleConfig = { dictionary_id: selectedDictId, dictionary_name: dict?.name ?? '', match_mode: matchMode, case_sensitive: caseSensitive }
      }
      const payload = { name: name.trim(), pattern: finalPattern, replacement: replacement.trim() || undefined, severity, category, description: description.trim() || undefined, rule_type: ruleType, rule_config: ruleConfig, classification_level: classificationLevel || undefined }
      if (isEditing) { await api.put(`/dlp-rules/${editingRule.id}`, payload); setMsg({ type: 'success', text: '规则更新成功' }) }
      else { await api.post('/dlp-rules', payload); setMsg({ type: 'success', text: '规则创建成功' }) }
      setTimeout(() => { onSaved(); onClose() }, 500)
    } catch (err: any) { setMsg({ type: 'error', text: err?.response?.data?.error || '操作失败' }) }
    finally { setSaving(false) }
  }

  const inputCls = 'w-full bg-[#F5F5F5] px-3.5 py-2.5 font-mono text-[10px] font-medium text-[#1A1A1A] outline-none placeholder:text-[#CCCCCC]'
  const inputBorder = { border: '1px solid #E8E8E8' }

  return (
    <Dialog open={open} onOpenChange={(v) => { if (!v) onClose() }}>
      <DialogContent className="sm:max-w-[600px] rounded-none p-0 gap-0 ring-0" style={{ border: '1px solid #E8E8E8' }} showCloseButton={false}>
        <div className="flex items-center justify-between px-6 py-5" style={{ borderBottom: '1px solid #E8E8E8' }}>
          <div className="flex items-center gap-2.5">
            <Shield className="h-4 w-4 text-[#0A6B3A]" />
            <span className="text-base font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>{isEditing ? '编辑 DLP 规则' : '新建 DLP 规则'}</span>
          </div>
          <button onClick={onClose} className="font-mono text-sm font-semibold text-[#999999] hover:text-[#1A1A1A]">✕</button>
        </div>
        {msg && <div className="px-6 py-2" style={{ backgroundColor: msg.type === 'success' ? '#0A6B3A08' : '#CF132208', borderBottom: '1px solid #E8E8E8' }}><span className="font-mono text-[10px] font-medium" style={{ color: msg.type === 'success' ? '#0A6B3A' : '#CF1322' }}>{msg.text}</span></div>}
        <div className="flex flex-col gap-4 px-6 py-6" style={{ maxHeight: '55vh', overflowY: 'auto' }}>
          <div className="flex flex-col gap-2"><label className="font-mono text-[10px] font-semibold text-[#1A1A1A]">规则名称 <span className="text-[#CF1322]">*</span></label><input value={name} onChange={(e) => setName(e.target.value)} placeholder="例如：身份证号检测" className={inputCls} style={inputBorder} /></div>
          <div className="grid grid-cols-2 gap-3">
            <div className="flex flex-col gap-2"><label className="font-mono text-[10px] font-semibold text-[#1A1A1A]">规则类型</label><DropdownSelect label={RULE_TYPE_MAP[ruleType]?.label ?? '正则'} options={Object.entries(RULE_TYPE_MAP).map(([k, v]) => ({ value: k, label: v.label }))} value={ruleType} onChange={(v) => { setRuleType(v as any); setTestResult(null) }} /></div>
            <div className="flex flex-col gap-2"><label className="font-mono text-[10px] font-semibold text-[#1A1A1A]">严重级别 <span className="text-[#CF1322]">*</span></label><DropdownSelect label={SEVERITY_MAP[severity]?.label ?? '中危'} options={SEVERITY_OPTIONS} value={severity} onChange={setSeverity} /></div>
          </div>
          {ruleType === 'regex' && <div className="flex flex-col gap-2"><label className="font-mono text-[10px] font-semibold text-[#1A1A1A]">匹配模式 <span className="text-[#CF1322]">*</span></label><input value={pattern} onChange={(e) => handlePatternChange(e.target.value)} placeholder="\\d{17}[\\dXx]" className={inputCls} style={{ border: regexError ? '1px solid #CF1322' : '1px solid #E8E8E8' }} />{regexError ? <span className="font-mono text-[9px] font-medium text-[#CF1322]">{regexError}</span> : <span className="font-mono text-[9px] font-medium text-[#999999]">支持正则表达式</span>}</div>}
          {ruleType === 'keyword' && <div className="flex flex-col gap-2"><label className="font-mono text-[10px] font-semibold text-[#1A1A1A]">关键字列表 <span className="text-[#CF1322]">*</span></label><div className="flex flex-wrap gap-1.5 min-h-[28px]">{keywords.map((kw) => <span key={kw} className="inline-flex items-center gap-1 px-2 py-0.5 font-mono text-[9px] font-medium text-[#1A1A1A]" style={{ backgroundColor: '#F5F5F5', border: '1px solid #E8E8E8' }}>{kw}<button onClick={() => setKeywords(keywords.filter((k) => k !== kw))} className="text-[#999999] hover:text-[#CF1322]">×</button></span>)}{keywords.length === 0 && <span className="font-mono text-[9px] text-[#CCCCCC]">请添加至少一个关键字</span>}</div><div className="flex gap-2"><input value={keywordInput} onChange={(e) => setKeywordInput(e.target.value)} onKeyDown={(e) => { if (e.key === 'Enter') { e.preventDefault(); addKeyword() } }} placeholder="输入关键字后按回车添加" className={inputCls + ' flex-1'} style={inputBorder} /><button onClick={addKeyword} className="shrink-0 bg-[#0A6B3A] px-3 py-2 font-mono text-[9px] font-semibold text-white">添加</button></div></div>}
          {ruleType === 'dictionary' && <div className="flex flex-col gap-2"><label className="font-mono text-[10px] font-semibold text-[#1A1A1A]">选择字典 <span className="text-[#CF1322]">*</span></label><DropdownSelect label={dictionaries.find((d) => d.id === selectedDictId)?.name ?? '请选择字典'} options={dictionaries.map((d) => ({ value: d.id, label: `${d.name} (${d.keyword_count} 个关键字)` }))} value={selectedDictId} onChange={setSelectedDictId} /></div>}
          {(ruleType === 'keyword' || ruleType === 'dictionary') && <div className="grid grid-cols-2 gap-3"><div className="flex flex-col gap-2"><label className="font-mono text-[10px] font-semibold text-[#1A1A1A]">匹配模式</label><DropdownSelect label={MATCH_MODE_OPTIONS.find((o) => o.value === matchMode)?.label ?? '包含匹配'} options={MATCH_MODE_OPTIONS} value={matchMode} onChange={setMatchMode} /></div><div className="flex flex-col gap-2"><label className="font-mono text-[10px] font-semibold text-[#1A1A1A]">区分大小写</label><div className="flex items-center gap-2 py-1.5"><ToggleSwitch on={caseSensitive} onChange={() => setCaseSensitive(!caseSensitive)} /><span className="font-mono text-[10px] font-medium text-[#999999]">{caseSensitive ? '是' : '否'}</span></div></div></div>}
          <div className="grid grid-cols-2 gap-3"><div className="flex flex-col gap-2"><label className="font-mono text-[10px] font-semibold text-[#1A1A1A]">替换文本</label><input value={replacement} onChange={(e) => setReplacement(e.target.value)} placeholder="默认: ***" className={inputCls} style={inputBorder} /></div><div className="flex flex-col gap-2"><label className="font-mono text-[10px] font-semibold text-[#1A1A1A]">分类 <span className="text-[#CF1322]">*</span></label><DropdownSelect label={CATEGORY_MAP[category] ?? 'PII'} options={CATEGORY_OPTIONS} value={category} onChange={setCategory} /></div></div>
          <div className="flex flex-col gap-2"><label className="font-mono text-[10px] font-semibold text-[#1A1A1A]">数据分级</label><select value={classificationLevel} onChange={(e) => setClassificationLevel(e.target.value)} className={inputCls} style={inputBorder}><option value="">未分级</option><option value="public">公开</option><option value="internal">内部</option><option value="confidential">机密</option><option value="top_secret">绝密</option></select></div>
          <div className="flex flex-col gap-2"><label className="font-mono text-[10px] font-semibold text-[#1A1A1A]">描述</label><textarea value={description} onChange={(e) => setDescription(e.target.value)} placeholder="规则描述（可选）" rows={2} className="w-full resize-none bg-[#F5F5F5] px-3.5 py-2.5 font-mono text-[10px] font-medium text-[#1A1A1A] outline-none placeholder:text-[#CCCCCC]" style={inputBorder} /></div>
          {!showTest ? <button onClick={() => setShowTest(true)} className="w-full border border-dashed border-[#E8E8E8] bg-white py-2.5 font-mono text-[10px] font-medium text-[#999999] hover:border-[#0A6B3A] hover:text-[#0A6B3A]">🧪 测试此规则</button> : (
            <div className="flex flex-col gap-2 border border-[#E8E8E8] bg-[#FAFAFA] p-3">
              <div className="flex items-center justify-between"><span className="font-mono text-[10px] font-semibold text-[#1A1A1A]">规则测试</span><button onClick={() => { setShowTest(false); setTestResult(null); setTestText('') }} className="font-mono text-[9px] text-[#999999]">收起</button></div>
              <textarea value={testText} onChange={(e) => { setTestText(e.target.value); setTestResult(null) }} placeholder="输入测试文本..." rows={2} className="w-full resize-none bg-white px-3 py-2 font-mono text-[10px] text-[#1A1A1A] outline-none placeholder:text-[#CCCCCC]" style={inputBorder} />
              <button onClick={handleTest} className="self-start bg-[#0A6B3A] px-3 py-1.5 font-mono text-[9px] font-semibold text-white">执行测试</button>
              {testResult && <div className="flex flex-col gap-1 p-2" style={{ backgroundColor: testResult.matched ? '#0A6B3A08' : '#D4870008', border: `1px solid ${testResult.matched ? '#0A6B3A40' : '#D4870040'}` }}><span className="font-mono text-[9px] font-semibold" style={{ color: testResult.matched ? '#0A6B3A' : '#D48700' }}>{testResult.matched ? `✓ 匹配成功（${testResult.matches.length} 处）` : '✗ 未匹配到内容'}</span>{testResult.matched && <><div className="flex flex-wrap gap-1">{testResult.matches.slice(0, 5).map((m, i) => <span key={i} className="px-1.5 py-0.5 font-mono text-[8px] font-medium text-[#CF1322]" style={{ backgroundColor: '#CF13220D', border: '1px solid #CF132240' }}>{m}</span>)}{testResult.matches.length > 5 && <span className="font-mono text-[8px] text-[#999999]">+{testResult.matches.length - 5}</span>}</div><span className="font-mono text-[9px] text-[#999999]">替换结果：<code className="text-[#1A1A1A]">{testResult.replaced}</code></span></>}</div>}
            </div>
          )}
        </div>
        <div className="flex items-center justify-end gap-3 px-6 py-4" style={{ borderTop: '1px solid #E8E8E8' }}>
          <button onClick={onClose} className="border border-[#E8E8E8] bg-white px-5 py-2.5 font-mono text-[10px] font-semibold text-[#1A1A1A]">取消</button>
          <button onClick={handleSave} disabled={saving} className="flex items-center gap-2 bg-[#0A6B3A] px-5 py-2.5 font-mono text-[10px] font-semibold text-white disabled:opacity-50"><Check className="h-3 w-3" />{isEditing ? '保存' : '创建规则'}</button>
        </div>
      </DialogContent>
    </Dialog>
  )
}

/* ── 创建/编辑字典弹窗 ── */

function DictFormDialog({ open, onClose, onSaved }: { open: boolean; onClose: () => void; onSaved: () => void }) {
  const [name, setName] = useState('')
  const [description, setDescription] = useState('')
  const [keywordText, setKeywordText] = useState('')
  const [saving, setSaving] = useState(false)
  const [msg, setMsg] = useState<{ type: 'success' | 'error'; text: string } | null>(null)
  const fileInputRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    if (!open) return
    setName(''); setDescription(''); setKeywordText(''); setMsg(null)
  }, [open])

  const keywords = keywordText.split('\n').map((k) => k.trim()).filter(Boolean)
  const uniqueKeywords = [...new Set(keywords)]
  const duplicateCount = keywords.length - uniqueKeywords.length

  function handleFileImport(e: React.ChangeEvent<HTMLInputElement>) {
    const file = e.target.files?.[0]
    if (!file) return
    file.text().then((text) => {
      const imported = text.split('\n').map((k) => k.trim()).filter(Boolean)
      if (!imported.length) return
      const existing = keywordText.split('\n').map((k) => k.trim()).filter(Boolean)
      const merged = [...new Set([...existing, ...imported])]
      setKeywordText(merged.join('\n'))
    }).catch((err) => console.error('读取文件失败', err))
    if (fileInputRef.current) fileInputRef.current.value = ''
  }

  async function handleSave() {
    if (!name.trim()) { setMsg({ type: 'error', text: '请输入字典名称' }); return }
    if (uniqueKeywords.length === 0) { setMsg({ type: 'error', text: '请至少输入一个关键字' }); return }
    setSaving(true); setMsg(null)
    try {
      await api.post('/dlp-dictionaries', { name: name.trim(), description: description.trim() || undefined, keywords: uniqueKeywords })
      setMsg({ type: 'success', text: '字典创建成功' })
      setTimeout(() => { onSaved(); onClose() }, 500)
    } catch (err: any) {
      setMsg({ type: 'error', text: err?.response?.data?.error || '创建失败' })
    } finally { setSaving(false) }
  }

  const inputCls = 'w-full bg-[#F5F5F5] px-3.5 py-2.5 font-mono text-[10px] font-medium text-[#1A1A1A] outline-none placeholder:text-[#CCCCCC]'
  const inputBorder = { border: '1px solid #E8E8E8' }

  return (
    <Dialog open={open} onOpenChange={(v) => { if (!v) onClose() }}>
      <DialogContent className="sm:max-w-[560px] rounded-none p-0 gap-0 ring-0" style={{ border: '1px solid #E8E8E8' }} showCloseButton={false}>
        <div className="flex items-center justify-between px-6 py-5" style={{ borderBottom: '1px solid #E8E8E8' }}>
          <div className="flex items-center gap-2.5">
            <BookOpen className="h-4 w-4 text-[#0A6B3A]" />
            <span className="text-base font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>创建字典</span>
          </div>
          <button onClick={onClose} className="font-mono text-sm font-semibold text-[#999999] hover:text-[#1A1A1A]">✕</button>
        </div>
        {msg && <div className="px-6 py-2" style={{ backgroundColor: msg.type === 'success' ? '#0A6B3A08' : '#CF132208', borderBottom: '1px solid #E8E8E8' }}><span className="font-mono text-[10px] font-medium" style={{ color: msg.type === 'success' ? '#0A6B3A' : '#CF1322' }}>{msg.text}</span></div>}
        <div className="flex flex-col gap-4 px-6 py-6" style={{ maxHeight: '55vh', overflowY: 'auto' }}>
          <div className="flex flex-col gap-2">
            <label className="font-mono text-[10px] font-semibold text-[#1A1A1A]">字典名称 <span className="text-[#CF1322]">*</span></label>
            <input value={name} onChange={(e) => setName(e.target.value)} placeholder="例如：敏感词字典" className={inputCls} style={inputBorder} />
          </div>
          <div className="flex flex-col gap-2">
            <label className="font-mono text-[10px] font-semibold text-[#1A1A1A]">描述</label>
            <input value={description} onChange={(e) => setDescription(e.target.value)} placeholder="字典用途说明（可选）" className={inputCls} style={inputBorder} />
          </div>
          <div className="flex flex-col gap-2">
            <div className="flex items-center gap-2">
              <label className="font-mono text-[10px] font-semibold text-[#1A1A1A]">关键字列表 <span className="text-[#CF1322]">*</span></label>
              <StatusTag label={`${uniqueKeywords.length} 个关键字`} color="#1677FF" />
              {duplicateCount > 0 && (
                <>
                  <StatusTag label={`${duplicateCount} 个重复`} color="#D48700" />
                  <button onClick={() => setKeywordText(uniqueKeywords.join('\n'))} className="font-mono text-[9px] font-medium text-[#0A6B3A]">一键去重</button>
                </>
              )}
            </div>
            <div className="flex gap-2">
              <button onClick={() => fileInputRef.current?.click()} className="flex items-center gap-1.5 border border-[#E8E8E8] bg-white px-3 py-1.5 font-mono text-[9px] font-semibold text-[#1A1A1A]">
                <Upload className="h-3 w-3 text-[#6a6a6a]" />
                从文件导入
              </button>
              <input ref={fileInputRef} type="file" accept=".txt,.csv" className="hidden" onChange={handleFileImport} />
            </div>
            <textarea
              value={keywordText}
              onChange={(e) => setKeywordText(e.target.value)}
              placeholder={'每行一个关键字，例如：\n机密\n绝密\n内部文件'}
              rows={10}
              className="w-full resize-none bg-[#F5F5F5] px-3.5 py-2.5 font-mono text-[10px] font-medium text-[#1A1A1A] outline-none placeholder:text-[#CCCCCC]"
              style={inputBorder}
            />
            <span className="font-mono text-[9px] font-medium text-[#999999]">每行输入一个关键字，空行自动忽略，提交时自动去重</span>
          </div>
        </div>
        <div className="flex items-center justify-end gap-3 px-6 py-4" style={{ borderTop: '1px solid #E8E8E8' }}>
          <button onClick={onClose} className="border border-[#E8E8E8] bg-white px-5 py-2.5 font-mono text-[10px] font-semibold text-[#1A1A1A]">取消</button>
          <button onClick={handleSave} disabled={saving} className="flex items-center gap-2 bg-[#0A6B3A] px-5 py-2.5 font-mono text-[10px] font-semibold text-white disabled:opacity-50"><Check className="h-3 w-3" />创建</button>
        </div>
      </DialogContent>
    </Dialog>
  )
}

/* ── 下拉选择器 ── */

function DropdownSelect({ label, options, value, onChange }: {
  label: string
  options: { value: string; label: string }[]
  value: string
  onChange: (v: string) => void
}) {
  const [open, setOpen] = useState(false)
  return (
    <div className="relative">
      <button onClick={() => setOpen(!open)} className="flex w-full items-center justify-between bg-[#F5F5F5] px-3.5 py-2.5" style={{ border: '1px solid #E8E8E8' }}>
        <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{label}</span>
        <ChevronDown className="h-3 w-3 text-[#999999]" />
      </button>
      {open && (
        <div className="absolute z-50 mt-1 w-full border border-[#E8E8E8] bg-white shadow-lg" style={{ maxHeight: 180, overflowY: 'auto' }}>
          {options.map((o) => (
            <button key={o.value} onClick={() => { onChange(o.value); setOpen(false) }} className="w-full px-3.5 py-2 text-left font-mono text-[10px] font-medium text-[#1A1A1A] hover:bg-[#FAFAFA]" style={{ backgroundColor: value === o.value ? 'rgba(10,107,58,0.06)' : undefined }}>
              {o.label}
            </button>
          ))}
        </div>
      )}
    </div>
  )
}

/* ── 新建/编辑敏感操作弹窗 ── */

interface SensitiveOpFormDialogProps {
  open: boolean
  editingOp: SensitiveOperation | null
  onClose: () => void
  onSaved: () => void
}

function SensitiveOpFormDialog({ open, editingOp, onClose, onSaved }: SensitiveOpFormDialogProps) {
  const isEditing = !!editingOp
  const [name, setName] = useState('')
  const [operationType, setOperationType] = useState('')
  const [riskLevel, setRiskLevel] = useState('medium')
  const [requiresApproval, setRequiresApproval] = useState(true)
  const [description, setDescription] = useState('')
  const [saving, setSaving] = useState(false)
  const [msg, setMsg] = useState<{ type: 'success' | 'error'; text: string } | null>(null)

  useEffect(() => {
    if (!open) return
    setMsg(null)
    if (editingOp) {
      setName(editingOp.name); setOperationType(editingOp.operation_type)
      setRiskLevel(editingOp.risk_level); setRequiresApproval(editingOp.requires_approval)
      setDescription(editingOp.description ?? '')
    } else {
      setName(''); setOperationType(''); setRiskLevel('medium'); setRequiresApproval(true); setDescription('')
    }
  }, [open, editingOp])

  async function handleSave() {
    if (!name.trim()) { setMsg({ type: 'error', text: '请输入操作名称' }); return }
    if (!operationType.trim()) { setMsg({ type: 'error', text: '请输入操作类型' }); return }
    setSaving(true); setMsg(null)
    try {
      const payload = { name: name.trim(), operation_type: operationType.trim(), risk_level: riskLevel, requires_approval: requiresApproval, description: description.trim() || undefined }
      if (isEditing) {
        await api.put(`/sensitive-operations/${editingOp.id}`, payload)
        setMsg({ type: 'success', text: '操作更新成功' })
      } else {
        await api.post('/sensitive-operations', payload)
        setMsg({ type: 'success', text: '操作创建成功' })
      }
      setTimeout(() => { onSaved(); onClose() }, 500)
    } catch (err: any) {
      setMsg({ type: 'error', text: err?.response?.data?.error || '操作失败' })
    } finally { setSaving(false) }
  }

  const RISK_OPTIONS = Object.entries(RISK_LEVEL_MAP).map(([k, v]) => ({ value: k, label: v.label }))
  const inputCls = 'w-full bg-[#F5F5F5] px-3.5 py-2.5 font-mono text-[10px] font-medium text-[#1A1A1A] outline-none placeholder:text-[#CCCCCC]'
  const inputBorder = { border: '1px solid #E8E8E8' }

  return (
    <Dialog open={open} onOpenChange={(v) => { if (!v) onClose() }}>
      <DialogContent className="sm:max-w-[480px] rounded-none p-0 gap-0 ring-0" style={{ border: '1px solid #E8E8E8' }} showCloseButton={false}>
        <div className="flex items-center justify-between px-6 py-5" style={{ borderBottom: '1px solid #E8E8E8' }}>
          <div className="flex items-center gap-2.5">
            <AlertTriangle className="h-4 w-4 text-[#D48700]" />
            <span className="text-base font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>{isEditing ? '编辑敏感操作' : '新建敏感操作'}</span>
          </div>
          <button onClick={onClose} className="font-mono text-sm font-semibold text-[#999999] hover:text-[#1A1A1A]">✕</button>
        </div>
        {msg && (
          <div className="px-6 py-2" style={{ backgroundColor: msg.type === 'success' ? '#0A6B3A08' : '#CF132208', borderBottom: '1px solid #E8E8E8' }}>
            <span className="font-mono text-[10px] font-medium" style={{ color: msg.type === 'success' ? '#0A6B3A' : '#CF1322' }}>{msg.text}</span>
          </div>
        )}
        <div className="flex flex-col gap-4 px-6 py-6">
          <div className="flex flex-col gap-2">
            <label className="font-mono text-[10px] font-semibold text-[#1A1A1A]">操作名称 <span className="text-[#CF1322]">*</span></label>
            <input value={name} onChange={(e) => setName(e.target.value)} placeholder="例如：文件导出" className={inputCls} style={inputBorder} />
          </div>
          <div className="flex flex-col gap-2">
            <label className="font-mono text-[10px] font-semibold text-[#1A1A1A]">操作类型 <span className="text-[#CF1322]">*</span></label>
            <input value={operationType} onChange={(e) => setOperationType(e.target.value)} placeholder="例如：file_export" className={inputCls} style={inputBorder} />
            <span className="font-mono text-[9px] text-[#999999]">用于与客户端策略匹配，建议使用英文下划线格式</span>
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div className="flex flex-col gap-2">
              <label className="font-mono text-[10px] font-semibold text-[#1A1A1A]">风险等级</label>
              <DropdownSelect label={RISK_LEVEL_MAP[riskLevel]?.label ?? '中'} options={RISK_OPTIONS} value={riskLevel} onChange={setRiskLevel} />
            </div>
            <div className="flex flex-col gap-2">
              <label className="font-mono text-[10px] font-semibold text-[#1A1A1A]">需要审批</label>
              <div className="flex items-center gap-2 py-1.5">
                <ToggleSwitch on={requiresApproval} onChange={() => setRequiresApproval(!requiresApproval)} />
                <span className="font-mono text-[10px] font-medium text-[#999999]">{requiresApproval ? '是' : '否'}</span>
              </div>
            </div>
          </div>
          <div className="flex flex-col gap-2">
            <label className="font-mono text-[10px] font-semibold text-[#1A1A1A]">描述</label>
            <textarea value={description} onChange={(e) => setDescription(e.target.value)} placeholder="操作描述（可选）" rows={2} className="w-full resize-none bg-[#F5F5F5] px-3.5 py-2.5 font-mono text-[10px] font-medium text-[#1A1A1A] outline-none placeholder:text-[#CCCCCC]" style={inputBorder} />
          </div>
        </div>
        <div className="flex items-center justify-end gap-3 px-6 py-4" style={{ borderTop: '1px solid #E8E8E8' }}>
          <button onClick={onClose} className="border border-[#E8E8E8] bg-white px-5 py-2.5 font-mono text-[10px] font-semibold text-[#1A1A1A]">取消</button>
          <button onClick={handleSave} disabled={saving} className="flex items-center gap-2 bg-[#0A6B3A] px-5 py-2.5 font-mono text-[10px] font-semibold text-white disabled:opacity-50">
            <Check className="h-3 w-3" />{isEditing ? '保存' : '创建'}
          </button>
        </div>
      </DialogContent>
    </Dialog>
  )
}

/* ── Tab 定义 ── */

const tabs = ['DLP 规则', '敏感词典', '策略版本', '拦截记录'] as const
type TabKey = (typeof tabs)[number]

const PAGE_SIZE = 10

/* ── 主组件 ── */

export default function SecurityPage() {
  const [activeTab, setActiveTab] = useState<TabKey>('DLP 规则')
  const [createDialogOpen, setCreateDialogOpen] = useState(false)
  const [editingRule, setEditingRule] = useState<DlpRule | null>(null)
  const [dictDialogOpen, setDictDialogOpen] = useState(false)
  const [sensitiveOpDialogOpen, setSensitiveOpDialogOpen] = useState(false)
  const [editingOp, setEditingOp] = useState<SensitiveOperation | null>(null)

  // DLP 规则
  const [rules, setRules] = useState<DlpRule[]>([])
  const [rulesLoading, setRulesLoading] = useState(false)
  const [ruleSearch, setRuleSearch] = useState('')
  const [ruleSeverity, setRuleSeverity] = useState('')
  const [ruleCategory, setRuleCategory] = useState('')
  const [rulePage, setRulePage] = useState(1)

  // 敏感词典
  const [dicts, setDicts] = useState<DlpDictionary[]>([])
  const [dictsLoading, setDictsLoading] = useState(false)
  const [dictPage, setDictPage] = useState(1)

  // 敏感操作
  const [ops, setOps] = useState<SensitiveOperation[]>([])
  const [opsLoading, setOpsLoading] = useState(false)
  const [opsPage, setOpsPage] = useState(1)

  // 策略版本
  const [changes, setChanges] = useState<PolicyChangeRecord[]>([])
  const [changesTotal, setChangesTotal] = useState(0)
  const [changesLoading, setChangesLoading] = useState(false)
  const [changesPage, setChangesPage] = useState(1)

  /* ── 数据加载 ── */

  const loadRules = useCallback(async () => {
    setRulesLoading(true)
    try {
      const resp = await api.get('/dlp-rules')
      setRules(resp.data.rules ?? [])
    } catch (err) {
      console.error('加载 DLP 规则失败:', err)
      setRules([])
    } finally {
      setRulesLoading(false)
    }
  }, [])

  const loadDicts = useCallback(async () => {
    setDictsLoading(true)
    try {
      const resp = await api.get('/dlp-dictionaries')
      setDicts(resp.data.dictionaries ?? [])
    } catch (err) {
      console.error('加载敏感词典失败:', err)
      setDicts([])
    } finally {
      setDictsLoading(false)
    }
  }, [])

  const loadOps = useCallback(async () => {
    setOpsLoading(true)
    try {
      const resp = await api.get('/sensitive-operations')
      setOps(resp.data.operations ?? [])
    } catch (err) {
      console.error('加载敏感操作失败:', err)
      setOps([])
    } finally {
      setOpsLoading(false)
    }
  }, [])

  const loadChanges = useCallback(async () => {
    setChangesLoading(true)
    try {
      const resp = await api.get('/policy-changes', { params: { page: changesPage, page_size: PAGE_SIZE } })
      setChanges(resp.data.records ?? [])
      setChangesTotal(resp.data.total ?? 0)
    } catch (err) {
      console.error('加载策略变更失败:', err)
      setChanges([])
      setChangesTotal(0)
    } finally {
      setChangesLoading(false)
    }
  }, [changesPage])

  useEffect(() => {
    if (activeTab === 'DLP 规则') loadRules()
    else if (activeTab === '敏感词典') loadDicts()
    else if (activeTab === '敏感操作') loadOps()
    else if (activeTab === '策略版本') loadChanges()
  }, [activeTab, loadRules, loadDicts, loadOps, loadChanges])

  /* ── 操作 ── */

  async function handleToggleRule(rule: DlpRule) {
    try {
      await api.put(`/dlp-rules/${rule.id}`, { enabled: !rule.enabled })
      loadRules()
    } catch (err) { console.error('切换规则状态失败:', err) }
  }

  async function handleDeleteRule(rule: DlpRule) {
    if (!confirm(`确定要删除规则「${rule.name}」吗？`)) return
    try {
      await api.delete(`/dlp-rules/${rule.id}`)
      loadRules()
    } catch (err) { console.error('删除规则失败:', err) }
  }

  async function handleDeleteDict(dict: DlpDictionary) {
    if (!confirm(`确定要删除词典「${dict.name}」吗？`)) return
    try {
      await api.delete(`/dlp-dictionaries/${dict.id}`)
      loadDicts()
    } catch (err) { console.error('删除词典失败:', err) }
  }

  async function handleToggleOp(op: SensitiveOperation) {
    try {
      await api.put(`/sensitive-operations/${op.id}`, { enabled: !op.enabled })
      loadOps()
    } catch (err) { console.error('切换操作状态失败:', err) }
  }

  async function handleDeleteOp(op: SensitiveOperation) {
    if (!confirm(`确定要删除操作「${op.name}」吗？`)) return
    try {
      await api.delete(`/sensitive-operations/${op.id}`)
      loadOps()
    } catch (err) { console.error('删除操作失败:', err) }
  }

  function handleTabChange(tab: TabKey) {
    setActiveTab(tab)
    setRulePage(1)
    setDictPage(1)
    setOpsPage(1)
    setChangesPage(1)
  }

  /* ── DLP 规则过滤 ── */

  const filteredRules = rules.filter((r) => {
    if (ruleSearch) {
      const kw = ruleSearch.toLowerCase()
      if (!r.name.toLowerCase().includes(kw) && !(r.description ?? '').toLowerCase().includes(kw)) return false
    }
    if (ruleSeverity && r.severity !== ruleSeverity) return false
    if (ruleCategory && r.category !== ruleCategory) return false
    return true
  })

  const pagedRules = filteredRules.slice((rulePage - 1) * PAGE_SIZE, rulePage * PAGE_SIZE)
  const pagedDicts = dicts.slice((dictPage - 1) * PAGE_SIZE, dictPage * PAGE_SIZE)
  const pagedOps = ops.slice((opsPage - 1) * PAGE_SIZE, opsPage * PAGE_SIZE)

  /* ── 空状态 / 加载状态 ── */

  function LoadingState() {
    return (
      <div className="flex flex-col items-center justify-center gap-3 py-16">
        <Loader2 className="h-6 w-6 animate-spin text-[#CCCCCC]" />
        <span className="font-mono text-[10px] font-medium text-[#999999]">加载中...</span>
      </div>
    )
  }

  function EmptyState({ icon: Icon, text }: { icon: React.ElementType; text: string }) {
    return (
      <div className="flex flex-col items-center justify-center gap-3 py-16">
        <Icon className="h-10 w-10 text-[#CCCCCC]" />
        <span className="font-mono text-[10px] font-medium text-[#999999]">{text}</span>
      </div>
    )
  }

  return (
    <div className="flex flex-col gap-6">

      {/* 标题区 */}
      <div className="flex items-end justify-between">
        <div className="flex flex-col gap-2">
          <h1 className="text-[22px] font-bold tracking-tight text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px' }}>
            安全策略
          </h1>
          <p className="font-mono text-xs text-[#999999]">DLP 规则、敏感词典与策略版本管理</p>
        </div>
        <div className="flex gap-2">
          <button className="flex items-center gap-2 border border-[#E8E8E8] bg-white px-4 py-2.5 font-mono text-[9px] font-semibold text-[#1A1A1A]">
            <Upload className="h-3 w-3 text-[#6a6a6a]" />
            导入
          </button>
          <button onClick={() => setCreateDialogOpen(true)} className="flex items-center gap-2 bg-[#0A6B3A] px-4 py-2.5 font-mono text-[9px] font-semibold text-white">
            <Plus className="h-3 w-3" />
            新建规则
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

      {/* ── DLP 规则 Tab ── */}
      {activeTab === 'DLP 规则' && (
        <>
          {/* 双向扫描横幅 */}
          <div
            className="flex items-center justify-between px-4 py-3"
            style={{ backgroundColor: 'rgba(10,107,58,0.08)', border: '1px solid rgba(10,107,58,0.25)' }}
          >
            <span className="font-mono text-[10px] font-medium text-[#0A6B3A]">
              双向扫描已启用 — 输入和输出均受 DLP 引擎保护
            </span>
            <span className="inline-block px-2 py-0.5 font-mono text-[8px] font-semibold" style={{ color: '#0A6B3A', backgroundColor: 'rgba(10,107,58,0.18)', border: '1px solid #0A6B3A' }}>
              新增
            </span>
          </div>

          {/* 筛选栏 */}
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <div className="flex w-[220px] items-center gap-2.5 border border-[#E8E8E8] bg-white px-3.5 py-2">
                <Search className="h-3 w-3 text-[#6a6a6a]" />
                <input
                  value={ruleSearch}
                  onChange={(e) => { setRuleSearch(e.target.value); setRulePage(1) }}
                  placeholder="搜索规则名称..."
                  className="w-full bg-transparent font-mono text-[10px] font-medium text-[#1A1A1A] outline-none placeholder:text-[#CCCCCC]"
                />
              </div>
              <DropdownSelect label={ruleSeverity ? (SEVERITY_MAP[ruleSeverity]?.label ?? '严重级别') : '严重级别'} options={SEVERITY_OPTIONS} value={ruleSeverity} onChange={(v) => { setRuleSeverity(v); setRulePage(1) }} />
              <DropdownSelect label={ruleCategory ? (CATEGORY_MAP[ruleCategory] ?? '分类') : '分类'} options={CATEGORY_OPTIONS} value={ruleCategory} onChange={(v) => { setRuleCategory(v); setRulePage(1) }} />
            </div>
            <span className="font-mono text-[10px] font-medium text-[#999999]">共 {filteredRules.length} 条规则</span>
          </div>

          {/* 表格 */}
          <div className="bg-white" style={{ border: '1px solid #E8E8E8' }}>
            <div className="flex items-center px-4 py-2.5" style={{ borderBottom: '1px solid #E8E8E8', backgroundColor: '#FAFAFA' }}>
              {['规则名称', '类型', '严重级别', '分类', '状态', '操作'].map((h) => (
                <span key={h} className="flex-1 min-w-0 font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]" style={h === '操作' ? { width: 80, flex: 'none' } : undefined}>{h}</span>
              ))}
            </div>

            {rulesLoading ? <LoadingState /> : pagedRules.length === 0 ? (
              <EmptyState icon={Shield} text={ruleSearch || ruleSeverity || ruleCategory ? '没有匹配的规则' : '暂无 DLP 规则'} />
            ) : pagedRules.map((r, i) => {
              const sev = SEVERITY_MAP[r.severity] ?? { label: r.severity, color: '#999' }
              const rt = RULE_TYPE_MAP[r.rule_type] ?? { label: r.rule_type, color: '#999' }
              return (
                <div key={r.id} className="flex items-center px-4 py-3" style={{ borderBottom: i < pagedRules.length - 1 ? '1px solid #E8E8E8' : 'none' }}>
                  <span className="flex-1 min-w-0 font-mono text-[10px] font-medium text-[#1A1A1A]">{r.name}</span>
                  <div className="flex-1 min-w-0"><StatusTag label={rt.label} color={rt.color} /></div>
                  <div className="flex-1 min-w-0"><StatusTag label={sev.label} color={sev.color} /></div>
                  <span className="flex-1 min-w-0 font-mono text-[10px] font-medium text-[#999999]">{CATEGORY_MAP[r.category] ?? r.category}</span>
                  <div className="flex-1 min-w-0"><ToggleSwitch on={r.enabled} onChange={() => handleToggleRule(r)} /></div>
                  <div className="flex items-center gap-3" style={{ width: 80, flexShrink: 0 }}>
                    <button className="text-[#6a6a6a] hover:text-[#0A6B3A]" title="编辑"><Pencil className="h-3.5 w-3.5" /></button>
                    <button onClick={() => handleDeleteRule(r)} className="text-[#6a6a6a] hover:text-[#CF1322]" title="删除"><Trash2 className="h-3.5 w-3.5" /></button>
                  </div>
                </div>
              )
            })}

            <TablePagination current={rulePage} total={filteredRules.length} pageSize={PAGE_SIZE} onChange={setRulePage} />
          </div>
        </>
      )}

      {/* ── 敏感词典 Tab ── */}
      {activeTab === '敏感词典' && (
        <>
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <div className="flex w-[220px] items-center gap-2.5 border border-[#E8E8E8] bg-white px-3.5 py-2">
                <Search className="h-3 w-3 text-[#6a6a6a]" />
                <span className="font-mono text-[10px] font-medium text-[#CCCCCC]">搜索字典名称...</span>
              </div>
            </div>
            <div className="flex items-center gap-3">
              <span className="font-mono text-[10px] font-medium text-[#999999]">共 {dicts.length} 个字典</span>
              <button onClick={() => setDictDialogOpen(true)} className="flex items-center gap-2 bg-[#0A6B3A] px-4 py-2 font-mono text-[9px] font-semibold text-white">
                <Plus className="h-3 w-3" />
                创建字典
              </button>
            </div>
          </div>
          <div className="bg-white" style={{ border: '1px solid #E8E8E8' }}>
          <div className="flex items-center px-4 py-2.5" style={{ borderBottom: '1px solid #E8E8E8', backgroundColor: '#FAFAFA' }}>
            {['字典名称', '描述', '关键字数量', '更新时间', '操作'].map((h) => (
              <span key={h} className="flex-1 min-w-0 font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]" style={h === '操作' ? { width: 80, flex: 'none' } : undefined}>{h}</span>
            ))}
          </div>

          {dictsLoading ? <LoadingState /> : pagedDicts.length === 0 ? (
            <EmptyState icon={BookOpen} text="暂无敏感词典" />
          ) : pagedDicts.map((d, i) => (
            <div key={d.id} className="flex items-center px-4 py-3" style={{ borderBottom: i < pagedDicts.length - 1 ? '1px solid #E8E8E8' : 'none' }}>
              <span className="flex-1 min-w-0 font-mono text-[10px] font-medium text-[#1A1A1A]">{d.name}</span>
              <span className="flex-1 min-w-0 font-mono text-[10px] font-medium text-[#999999]">{d.description || '—'}</span>
              <span className="flex-1 min-w-0 font-mono text-[10px] font-medium text-[#1A1A1A]">{d.keyword_count} 个</span>
              <span className="flex-1 min-w-0 font-mono text-[10px] font-medium text-[#999999]">{new Date(d.updated_at).toLocaleString('zh-CN')}</span>
              <div className="flex items-center gap-3" style={{ width: 80, flexShrink: 0 }}>
                <button className="text-[#6a6a6a] hover:text-[#0A6B3A]" title="编辑"><Pencil className="h-3.5 w-3.5" /></button>
                <button onClick={() => handleDeleteDict(d)} className="text-[#6a6a6a] hover:text-[#CF1322]" title="删除"><Trash2 className="h-3.5 w-3.5" /></button>
              </div>
            </div>
          ))}

          <TablePagination current={dictPage} total={dicts.length} pageSize={PAGE_SIZE} onChange={setDictPage} />
          </div>
        </>
      )}

      {/* ── 敏感操作 Tab ── */}
      {activeTab === '敏感操作' && (
        <>
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <div className="flex w-[220px] items-center gap-2.5 border border-[#E8E8E8] bg-white px-3.5 py-2">
                <Search className="h-3 w-3 text-[#6a6a6a]" />
                <span className="font-mono text-[10px] font-medium text-[#CCCCCC]">搜索敏感操作...</span>
              </div>
            </div>
            <div className="flex items-center gap-3">
              <span className="font-mono text-[10px] font-medium text-[#999999]">共 {ops.length} 条规则</span>
              <button onClick={() => setSensitiveOpDialogOpen(true)} className="flex items-center gap-2 bg-[#0A6B3A] px-4 py-2 font-mono text-[9px] font-semibold text-white">
                <Plus className="h-3 w-3" />
                新建操作
              </button>
            </div>
          </div>
          <div className="bg-white" style={{ border: '1px solid #E8E8E8' }}>
            <div className="flex items-center px-4 py-2.5" style={{ borderBottom: '1px solid #E8E8E8', backgroundColor: '#FAFAFA' }}>
              {['操作名称', '风险等级', '需审批', '状态', '操作'].map((h) => (
                <span key={h} className="flex-1 min-w-0 font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]" style={h === '操作' ? { width: 80, flex: 'none' } : undefined}>{h}</span>
              ))}
            </div>

            {opsLoading ? <LoadingState /> : pagedOps.length === 0 ? (
              <EmptyState icon={AlertTriangle} text="暂无敏感操作" />
            ) : pagedOps.map((op, i) => {
              const risk = RISK_LEVEL_MAP[op.risk_level] ?? { label: op.risk_level, color: '#999' }
              return (
                <div key={op.id} className="flex items-center px-4 py-3" style={{ borderBottom: i < pagedOps.length - 1 ? '1px solid #E8E8E8' : 'none' }}>
                  <span className="flex-1 min-w-0 font-mono text-[10px] font-medium text-[#1A1A1A]">{op.name}</span>
                  <div className="flex-1 min-w-0"><StatusTag label={risk.label} color={risk.color} /></div>
                  <span className="flex-1 min-w-0 font-mono text-[10px] font-medium" style={{ color: op.requires_approval ? '#0A6B3A' : '#999999' }}>
                    {op.requires_approval ? '是' : '否'}
                  </span>
                  <div className="flex-1 min-w-0"><ToggleSwitch on={op.enabled} onChange={() => handleToggleOp(op)} /></div>
                  <div className="flex items-center gap-3" style={{ width: 80, flexShrink: 0 }}>
                    <button onClick={() => { setEditingOp(op); setSensitiveOpDialogOpen(true) }} className="text-[#6a6a6a] hover:text-[#0A6B3A]" title="编辑"><Pencil className="h-3.5 w-3.5" /></button>
                    <button onClick={() => handleDeleteOp(op)} className="text-[#6a6a6a] hover:text-[#CF1322]" title="删除"><Trash2 className="h-3.5 w-3.5" /></button>
                  </div>
                </div>
              )
            })}

            <TablePagination current={opsPage} total={ops.length} pageSize={PAGE_SIZE} onChange={setOpsPage} />
          </div>
        </>
      )}

      {/* ── 策略版本 Tab ── */}
      {activeTab === '策略版本' && (
        <div className="bg-white" style={{ border: '1px solid #E8E8E8' }}>
          <div className="flex items-center px-4 py-2.5" style={{ borderBottom: '1px solid #E8E8E8', backgroundColor: '#FAFAFA' }}>
            {['时间', '操作人', '规则名', '变更类型', '规则类型'].map((h) => (
              <span key={h} className="flex-1 min-w-0 font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{h}</span>
            ))}
          </div>

          {changesLoading ? <LoadingState /> : changes.length === 0 ? (
            <EmptyState icon={History} text="暂无策略变更记录" />
          ) : changes.map((c, i) => {
            const ct = CHANGE_TYPE_MAP[c.change_type] ?? { label: c.change_type, color: '#999' }
            const rt = RULE_TYPE_MAP[c.rule_type] ?? { label: c.rule_type, color: '#999' }
            return (
              <div key={c.id} className="flex items-center px-4 py-3" style={{ borderBottom: i < changes.length - 1 ? '1px solid #E8E8E8' : 'none' }}>
                <span className="flex-1 min-w-0 font-mono text-[10px] font-medium text-[#999999]">{new Date(c.changed_at).toLocaleString('zh-CN')}</span>
                <span className="flex-1 min-w-0 font-mono text-[10px] font-medium text-[#1A1A1A]">{c.changed_by_name || 'system'}</span>
                <span className="flex-1 min-w-0 font-mono text-[10px] font-medium text-[#1A1A1A]">{c.rule_name || c.rule_id.slice(0, 8)}</span>
                <div className="flex-1 min-w-0"><StatusTag label={ct.label} color={ct.color} /></div>
                <div className="flex-1 min-w-0"><StatusTag label={rt.label} color={rt.color} /></div>
              </div>
            )
          })}

          <TablePagination current={changesPage} total={changesTotal} pageSize={PAGE_SIZE} onChange={setChangesPage} />
        </div>
      )}

      {/* ── 拦截记录 Tab ── */}
      {activeTab === '拦截记录' && (
        <EmptyState icon={ShieldOff} text="拦截记录功能开发中，敬请期待" />
      )}

      <DlpRuleFormDialog open={createDialogOpen} editingRule={editingRule} onClose={() => { setCreateDialogOpen(false); setEditingRule(null) }} onSaved={loadRules} />
      <DictFormDialog open={dictDialogOpen} onClose={() => setDictDialogOpen(false)} onSaved={loadDicts} />
      <SensitiveOpFormDialog open={sensitiveOpDialogOpen} editingOp={editingOp} onClose={() => { setSensitiveOpDialogOpen(false); setEditingOp(null) }} onSaved={loadOps} />
    </div>
  )
}


