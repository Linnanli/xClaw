import { Search, Upload, Plus, Pencil, Trash2, ChevronDown } from 'lucide-react'
import { useState } from 'react'

/* ── Mock 数据 ── */

const tabs = ['DLP 规则', '敏感词典', '敏感操作', '策略版本', '拦截记录']

const severityMap: Record<string, { label: string; color: string }> = {
  critical: { label: '严重', color: '#CF1322' },
  high: { label: '高危', color: '#D48700' },
  medium: { label: '中危', color: '#D4B200' },
  low: { label: '低危', color: '#0A6B3A' },
}

const classificationMap: Record<string, { label: string; color: string }> = {
  top_secret: { label: '绝密', color: '#D48700' },
  secret: { label: '机密', color: '#CF1322' },
  internal: { label: '内部', color: '#D4B200' },
  public: { label: '公开', color: '#0A6B3A' },
}

const directionMap: Record<string, { label: string; color: string }> = {
  both: { label: '双向', color: '#0A6B3A' },
  input: { label: '输入', color: '#999999' },
  output: { label: '输出', color: '#999999' },
}

const mockRules = [
  {
    name: '身份证号码检测',
    severity: 'critical',
    classification: 'top_secret',
    direction: 'both',
    hitCount: '1,284',
    enabled: true,
  },
  {
    name: '银行卡号检测',
    severity: 'high',
    classification: 'secret',
    direction: 'both',
    hitCount: '847',
    enabled: true,
  },
  {
    name: '手机号码检测',
    severity: 'medium',
    classification: 'internal',
    direction: 'input',
    hitCount: '2,156',
    enabled: true,
  },
  {
    name: '邮箱地址检测',
    severity: 'low',
    classification: 'public',
    direction: 'output',
    hitCount: '3,891',
    enabled: false,
  },
]

/* ── 辅助：渲染彩色标签 ── */

function StatusTag({ label, color }: { label: string; color: string }) {
  return (
    <span
      className="inline-block px-2 py-0.5 font-mono text-[9px] font-semibold"
      style={{
        color,
        backgroundColor: `${color}1A`,
        border: `1px solid ${color}`,
      }}
    >
      {label}
    </span>
  )
}

/* ── 组件 ── */

export default function SecurityPage() {
  const [activeTab, setActiveTab] = useState('DLP 规则')

  return (
    <div className="flex flex-col gap-6">

      {/* 标题区 */}
      <div className="flex items-end justify-between">
        <div className="flex flex-col gap-2">
          <h1 className="text-[22px] font-bold tracking-tight text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px' }}>
            安全策略
          </h1>
          <p className="font-mono text-xs text-[#999999]">DLP 规则、敏感词典、敏感操作与策略版本管理</p>
        </div>
        <div className="flex gap-2">
          <button className="flex items-center gap-2 border border-[#E8E8E8] bg-white px-4 py-2.5 font-mono text-[9px] font-semibold text-[#1A1A1A]">
            <Upload className="h-3 w-3 text-[#6a6a6a]" />
            导入
          </button>
          <button className="flex items-center gap-2 bg-[#0A6B3A] px-4 py-2.5 font-mono text-[9px] font-semibold text-white">
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

      {/* 双向扫描横幅 */}
      <div
        className="flex items-center justify-between px-4 py-3"
        style={{ backgroundColor: 'rgba(10,107,58,0.08)', border: '1px solid rgba(10,107,58,0.25)' }}
      >
        <span className="font-mono text-[10px] font-medium text-[#0A6B3A]">
          双向扫描已启用 — 输入和输出均受 DLP 引擎保护
        </span>
        <span
          className="inline-block px-2 py-0.5 font-mono text-[8px] font-semibold"
          style={{ color: '#0A6B3A', backgroundColor: 'rgba(10,107,58,0.18)', border: '1px solid #0A6B3A' }}
        >
          新增
        </span>
      </div>

      {/* 筛选栏 */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="flex w-[220px] items-center gap-2.5 border border-[#E8E8E8] bg-white px-3.5 py-2">
            <Search className="h-3 w-3 text-[#6a6a6a]" />
            <span className="font-mono text-[10px] font-medium text-[#CCCCCC]">搜索规则名称...</span>
          </div>
          <div className="flex items-center gap-2 border border-[#E8E8E8] bg-white px-3.5 py-2">
            <span className="font-mono text-[10px] font-medium text-[#999999]">严重级别</span>
            <ChevronDown className="h-3 w-3 text-[#999999]" />
          </div>
          <div className="flex items-center gap-2 border border-[#E8E8E8] bg-white px-3.5 py-2">
            <span className="font-mono text-[10px] font-medium text-[#999999]">数据分级</span>
            <ChevronDown className="h-3 w-3 text-[#999999]" />
          </div>
        </div>
        <span className="font-mono text-[10px] font-medium text-[#999999]">共 24 条规则</span>
      </div>

      {/* 表格 */}
      <div className="bg-white" style={{ border: '1px solid #E8E8E8' }}>
        {/* 表头 */}
        <div className="grid grid-cols-7 px-4 py-2.5" style={{ borderBottom: '1px solid #E8E8E8' }}>
          {['规则名称', '级别', '数据分级', '扫描方向', '命中数', '状态', '操作'].map((h) => (
            <span key={h} className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{h}</span>
          ))}
        </div>

        {/* 数据行 */}
        {mockRules.map((r, i) => {
          const sev = severityMap[r.severity]
          const cls = classificationMap[r.classification]
          const dir = directionMap[r.direction]
          return (
            <div
              key={r.name}
              className="grid grid-cols-7 items-center px-4 py-3"
              style={{ borderBottom: i < mockRules.length - 1 ? '1px solid #E8E8E8' : 'none' }}
            >
              <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{r.name}</span>
              <div><StatusTag label={sev.label} color={sev.color} /></div>
              <div><StatusTag label={cls.label} color={cls.color} /></div>
              <div><StatusTag label={dir.label} color={dir.color} /></div>
              <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{r.hitCount}</span>
              {/* 状态 */}
              <div className="flex items-center gap-1.5">
                <div
                  className="h-2 w-2 rounded-full"
                  style={{ backgroundColor: r.enabled ? '#0A6B3A' : '#999999' }}
                />
                <span
                  className="font-mono text-[10px] font-medium"
                  style={{ color: r.enabled ? '#1A1A1A' : '#999999' }}
                >
                  {r.enabled ? '开启' : '关闭'}
                </span>
              </div>
              {/* 操作 */}
              <div className="flex items-center gap-3">
                <button className="text-[#6a6a6a] hover:text-[#0A6B3A]" title="编辑">
                  <Pencil className="h-3.5 w-3.5" />
                </button>
                <button className="text-[#6a6a6a] hover:text-[#CF1322]" title="删除">
                  <Trash2 className="h-3.5 w-3.5" />
                </button>
              </div>
            </div>
          )
        })}
      </div>
    </div>
  )
}
