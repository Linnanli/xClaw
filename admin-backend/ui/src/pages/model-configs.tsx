import { Plus } from 'lucide-react'

/* ── Mock 数据 ── */

const mockModels = [
  {
    name: 'DeepSeek-V3',
    provider: 'DeepSeek',
    apiFormat: 'OpenAI 兼容',
    capabilities: ['对话', '代码'],
    enabled: true,
    isDefault: true,
  },
  {
    name: 'Qwen-Max',
    provider: '阿里云',
    apiFormat: 'OpenAI 兼容',
    capabilities: ['对话', '推理'],
    enabled: true,
    isDefault: false,
  },
  {
    name: 'GPT-4o',
    provider: 'OpenAI',
    apiFormat: 'OpenAI',
    capabilities: ['对话', '视觉', '代码'],
    enabled: true,
    isDefault: false,
  },
  {
    name: 'Claude-3.5',
    provider: 'Anthropic',
    apiFormat: 'Anthropic',
    capabilities: ['对话', '代码', '分析'],
    enabled: false,
    isDefault: false,
  },
]

/* ── 组件 ── */

export default function ModelConfigsPage() {
  return (
    <div className="flex flex-col gap-6">

      {/* 标题区 */}
      <div className="flex items-end justify-between">
        <div className="flex flex-col gap-2">
          <h1 className="text-[22px] font-bold tracking-tight text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px' }}>
            模型配置
          </h1>
          <p className="font-mono text-xs text-[#999999]">管理多提供商 AI 模型的接入配置与调度</p>
        </div>
        <button className="flex items-center gap-2 bg-[#0A6B3A] px-4 py-2.5 font-mono text-[9px] font-semibold text-white">
          <Plus className="h-3 w-3" />
          添加模型
        </button>
      </div>

      {/* 表格 */}
      <div className="bg-white" style={{ border: '1px solid #E8E8E8' }}>
        {/* 表头 */}
        <div className="grid grid-cols-7 px-4 py-2.5" style={{ borderBottom: '1px solid #E8E8E8' }}>
          {['模型名称', '提供商', 'API 格式', '能力标签', '状态', '默认', '操作'].map((h) => (
            <span key={h} className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{h}</span>
          ))}
        </div>

        {/* 数据行 */}
        {mockModels.map((m, i) => (
          <div
            key={m.name}
            className="grid grid-cols-7 items-center px-4 py-3"
            style={{ borderBottom: i < mockModels.length - 1 ? '1px solid #E8E8E8' : 'none' }}
          >
            <span className="font-mono text-[10px] font-semibold text-[#1A1A1A]">{m.name}</span>
            <span className="font-mono text-[10px] font-medium text-[#999999]">{m.provider}</span>
            <span className="font-mono text-[10px] font-medium text-[#999999]">{m.apiFormat}</span>
            <div className="flex flex-wrap gap-1">
              {m.capabilities.map((cap) => (
                <span
                  key={cap}
                  className="inline-block px-1.5 py-0.5 font-mono text-[8px] font-semibold"
                  style={{ color: '#0A6B3A', backgroundColor: 'rgba(10,107,58,0.1)', border: '1px solid rgba(10,107,58,0.3)' }}
                >
                  {cap}
                </span>
              ))}
            </div>
            <div>
              <span
                className="inline-block px-2 py-0.5 font-mono text-[9px] font-semibold"
                style={{
                  color: m.enabled ? '#0A6B3A' : '#999999',
                  backgroundColor: m.enabled ? 'rgba(10,107,58,0.1)' : 'rgba(153,153,153,0.1)',
                  border: `1px solid ${m.enabled ? '#0A6B3A' : '#999999'}`,
                }}
              >
                {m.enabled ? '已启用' : '已禁用'}
              </span>
            </div>
            <span
              className="font-mono text-[10px] font-semibold"
              style={{ color: m.isDefault ? '#0A6B3A' : '#CCCCCC' }}
            >
              {m.isDefault ? '✓ 默认' : '—'}
            </span>
            <button className="font-mono text-[10px] font-semibold text-[#0A6B3A] text-left">编辑</button>
          </div>
        ))}
      </div>
    </div>
  )
}
