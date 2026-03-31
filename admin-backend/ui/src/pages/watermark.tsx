import { Save, Loader2 } from 'lucide-react'
import { useState, useEffect, useCallback } from 'react'
import { api } from '@/lib/api'
import { FormLabel, StatusMessage } from '@/components/ui/form-helpers'

const inputClass = 'border border-[#E8E8E8] px-3 py-2 font-mono text-[10px] outline-none'

const POSITION_OPTIONS = [
  { value: 'diagonal', label: '对角线' },
  { value: 'center', label: '居中' },
  { value: 'bottom_right', label: '右下角' },
  { value: 'tiled', label: '平铺' },
]

export default function WatermarkPage() {
  const [enabled, setEnabled] = useState(false)
  const [template, setTemplate] = useState('{username} · {department} · {datetime}')
  const [fontSize, setFontSize] = useState(14)
  const [opacity, setOpacity] = useState(0.15)
  const [position, setPosition] = useState('diagonal')
  const [color, setColor] = useState('#000000')
  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState(false)
  const [msg, setMsg] = useState<{ type: 'success' | 'error'; text: string } | null>(null)

  const fetchSettings = useCallback(async () => {
    setLoading(true)
    try {
      const { data } = await api.get('/settings')
      setEnabled(data.watermark_enabled ?? false)
      setTemplate(data.watermark_template ?? '{username} · {department} · {datetime}')
      setFontSize(data.watermark_font_size ?? 14)
      setOpacity(data.watermark_opacity ?? 0.15)
      setPosition(data.watermark_position ?? 'diagonal')
      setColor(data.watermark_color ?? '#000000')
    } catch { /* 静默 */ }
    setLoading(false)
  }, [])

  useEffect(() => { fetchSettings() }, [fetchSettings])

  const handleSave = async () => {
    setSaving(true); setMsg(null)
    try {
      await api.put('/settings', {
        watermark_enabled: enabled,
        watermark_template: template,
        watermark_font_size: fontSize,
        watermark_opacity: opacity,
        watermark_position: position,
        watermark_color: color,
      })
      setMsg({ type: 'success', text: '水印配置已保存' })
      fetchSettings()
    } catch { setMsg({ type: 'error', text: '保存失败' }) }
    setSaving(false)
  }

  // 预览文本
  const previewText = template
    .replace('{username}', 'zhang.wei')
    .replace('{department}', '研发部')
    .replace('{datetime}', new Date().toLocaleString('zh-CN'))

  if (loading) {
    return <div className="flex items-center justify-center py-32"><Loader2 className="h-5 w-5 animate-spin text-[#999]" /></div>
  }

  return (
    <div className="flex flex-col gap-6">
      <div className="flex items-end justify-between">
        <div className="flex flex-col gap-2">
          <h1 className="text-[22px] font-bold tracking-tight text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px' }}>
            水印管理
          </h1>
          <p className="font-mono text-xs text-[#999999]">配置导出文件的可见水印，威慑数据泄露行为</p>
        </div>
      </div>

      {msg && <StatusMessage type={msg.type} message={msg.text} />}

      <div className="flex gap-6">
        {/* 配置区 */}
        <div className="flex flex-1 flex-col gap-5 bg-white p-6" style={{ border: '1px solid #E8E8E8' }}>
          <div className="flex items-center justify-between pb-4" style={{ borderBottom: '1px solid #E8E8E8' }}>
            <span className="font-mono text-[11px] font-semibold tracking-[0.5px] text-[#1A1A1A]">水印配置</span>
            <div className="flex items-center gap-3">
              <span className="font-mono text-[10px] text-[#999]">{enabled ? '已启用' : '已禁用'}</span>
              <button onClick={() => setEnabled(!enabled)} className={`h-5 w-9 rounded-full transition-colors ${enabled ? 'bg-[#0A6B3A]' : 'bg-[#D9D9D9]'}`}>
                <div className={`h-4 w-4 rounded-full bg-white shadow transition-transform ${enabled ? 'translate-x-4' : 'translate-x-0.5'}`} />
              </button>
            </div>
          </div>

          <div className="flex flex-col gap-1.5">
            <FormLabel>内容模板</FormLabel>
            <input className={inputClass} value={template} onChange={(e) => setTemplate(e.target.value)} />
            <span className="font-mono text-[9px] text-[#999]">支持变量：{'{username}'} {'{department}'} {'{datetime}'}</span>
          </div>

          <div className="grid grid-cols-4 gap-4">
            <div className="flex flex-col gap-1.5">
              <FormLabel>字体大小</FormLabel>
              <input type="number" className={inputClass} value={fontSize} min={8} max={48} onChange={(e) => setFontSize(Number(e.target.value))} />
            </div>
            <div className="flex flex-col gap-1.5">
              <FormLabel>透明度</FormLabel>
              <input type="number" className={inputClass} value={opacity} min={0.01} max={1} step={0.05} onChange={(e) => setOpacity(Number(e.target.value))} />
            </div>
            <div className="flex flex-col gap-1.5">
              <FormLabel>颜色</FormLabel>
              <div className="flex items-center gap-2">
                <input type="color" value={color} onChange={(e) => setColor(e.target.value)} className="h-8 w-8 cursor-pointer border border-[#E8E8E8]" />
                <input className={`flex-1 ${inputClass}`} value={color} onChange={(e) => setColor(e.target.value)} />
              </div>
            </div>
            <div className="flex flex-col gap-1.5">
              <FormLabel>位置</FormLabel>
              <select className={inputClass} value={position} onChange={(e) => setPosition(e.target.value)}>
                {POSITION_OPTIONS.map((o) => <option key={o.value} value={o.value}>{o.label}</option>)}
              </select>
            </div>
          </div>

          <div className="flex justify-end pt-4" style={{ borderTop: '1px solid #E8E8E8' }}>
            <button onClick={handleSave} disabled={saving} className="flex items-center gap-1.5 bg-[#0A6B3A] px-4 py-2 font-mono text-[9px] font-semibold text-white disabled:opacity-50">
              {saving ? <Loader2 className="h-3 w-3 animate-spin" /> : <Save className="h-3 w-3" />}
              保存配置
            </button>
          </div>
        </div>

        {/* 预览区 */}
        <div className="flex w-[320px] flex-col gap-4 bg-white p-6" style={{ border: '1px solid #E8E8E8' }}>
          <span className="font-mono text-[11px] font-semibold tracking-[0.5px] text-[#1A1A1A]">预览效果</span>
          <div className="relative flex h-[400px] items-center justify-center overflow-hidden bg-[#FAFAFA]" style={{ border: '1px solid #E8E8E8' }}>
            {/* 模拟文档内容 */}
            <div className="flex flex-col gap-2 px-6">
              <div className="h-2 w-48 bg-[#E8E8E8]" />
              <div className="h-2 w-40 bg-[#E8E8E8]" />
              <div className="h-2 w-52 bg-[#E8E8E8]" />
              <div className="h-2 w-36 bg-[#E8E8E8]" />
            </div>
            {/* 水印叠加 */}
            {enabled && (
              <div className="pointer-events-none absolute inset-0 flex items-center justify-center"
                style={{ transform: position === 'diagonal' ? 'rotate(-30deg)' : 'none' }}>
                <span className="select-none font-mono font-semibold whitespace-nowrap"
                  style={{ fontSize: `${fontSize}px`, opacity, color }}>
                  {previewText}
                </span>
              </div>
            )}
          </div>
          {!enabled && <span className="font-mono text-[9px] text-[#999]">水印已禁用，启用后可预览效果</span>}
        </div>
      </div>
    </div>
  )
}
