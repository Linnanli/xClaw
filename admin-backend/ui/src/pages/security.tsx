import { Search, Upload, Plus, Pencil, Trash2, ChevronDown } from 'lucide-react'
import { useState } from 'react'
import { TablePagination } from '@/components/ui/table-pagination'
import { StatusTag } from '@/components/ui/status-tag'

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

/* ── 组件 ── */

export default function SecurityPage() {
  const [activeTab, setActiveTab] = useState('DLP 规则')
  const [currentPage, setCurrentPage] = useState(1)

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

      {/* Tab 内容区 */}
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
            <div className="grid grid-cols-7 px-4 py-2.5" style={{ borderBottom: '1px solid #E8E8E8' }}>
              {['规则名称', '级别', '数据分级', '扫描方向', '命中数', '状态', '操作'].map((h) => (
                <span key={h} className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{h}</span>
              ))}
            </div>
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
                  <div className="flex items-center gap-1.5">
                    <div className="h-2 w-2 rounded-full" style={{ backgroundColor: r.enabled ? '#0A6B3A' : '#999999' }} />
                    <span className="font-mono text-[10px] font-medium" style={{ color: r.enabled ? '#1A1A1A' : '#999999' }}>
                      {r.enabled ? '开启' : '关闭'}
                    </span>
                  </div>
                  <div className="flex items-center gap-3">
                    <button className="text-[#6a6a6a] hover:text-[#0A6B3A]" title="编辑"><Pencil className="h-3.5 w-3.5" /></button>
                    <button className="text-[#6a6a6a] hover:text-[#CF1322]" title="删除"><Trash2 className="h-3.5 w-3.5" /></button>
                  </div>
                </div>
              )
            })}
            <TablePagination current={currentPage} total={24} pageSize={10} onChange={setCurrentPage} />
          </div>
        </>
      )}

      {activeTab === '敏感词典' && (
        <div className="bg-white" style={{ border: '1px solid #E8E8E8' }}>
          <div className="grid grid-cols-5 px-4 py-2.5" style={{ borderBottom: '1px solid #E8E8E8' }}>
            {['词典名称', '词条数', '关联规则', '更新时间', '操作'].map((h) => (
              <span key={h} className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{h}</span>
            ))}
          </div>
          {[
            { name: '身份证号码库', count: '1,200', rules: 3, updated: '2024-07-08' },
            { name: '银行卡号库', count: '856', rules: 2, updated: '2024-07-05' },
            { name: '手机号码库', count: '2,400', rules: 2, updated: '2024-07-01' },
          ].map((d, i, arr) => (
            <div key={d.name} className="grid grid-cols-5 items-center px-4 py-3" style={{ borderBottom: i < arr.length - 1 ? '1px solid #E8E8E8' : 'none' }}>
              <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{d.name}</span>
              <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{d.count}</span>
              <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{d.rules} 条</span>
              <span className="font-mono text-[10px] font-medium text-[#999999]">{d.updated}</span>
              <div className="flex items-center gap-3">
                <button className="text-[#6a6a6a] hover:text-[#0A6B3A]" title="编辑"><Pencil className="h-3.5 w-3.5" /></button>
                <button className="text-[#6a6a6a] hover:text-[#CF1322]" title="删除"><Trash2 className="h-3.5 w-3.5" /></button>
              </div>
            </div>
          ))}
          <TablePagination current={currentPage} total={12} pageSize={10} onChange={setCurrentPage} />
        </div>
      )}

      {activeTab === '敏感操作' && (
        <div className="bg-white" style={{ border: '1px solid #E8E8E8' }}>
          <div className="grid grid-cols-5 px-4 py-2.5" style={{ borderBottom: '1px solid #E8E8E8' }}>
            {['操作名称', '风险等级', '需审批', '触发次数', '操作'].map((h) => (
              <span key={h} className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{h}</span>
            ))}
          </div>
          {[
            { name: '批量导出数据', risk: '高危', riskColor: '#CF1322', approval: true, count: '47' },
            { name: '删除对话记录', risk: '中危', riskColor: '#D48700', approval: true, count: '128' },
            { name: '修改 DLP 规则', risk: '高危', riskColor: '#CF1322', approval: true, count: '12' },
          ].map((op, i, arr) => (
            <div key={op.name} className="grid grid-cols-5 items-center px-4 py-3" style={{ borderBottom: i < arr.length - 1 ? '1px solid #E8E8E8' : 'none' }}>
              <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{op.name}</span>
              <div><StatusTag label={op.risk} color={op.riskColor} /></div>
              <span className="font-mono text-[10px] font-medium" style={{ color: op.approval ? '#0A6B3A' : '#999999' }}>{op.approval ? '是' : '否'}</span>
              <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{op.count}</span>
              <div className="flex items-center gap-3">
                <button className="text-[#6a6a6a] hover:text-[#0A6B3A]" title="编辑"><Pencil className="h-3.5 w-3.5" /></button>
              </div>
            </div>
          ))}
          <TablePagination current={currentPage} total={8} pageSize={10} onChange={setCurrentPage} />
        </div>
      )}

      {activeTab === '策略版本' && (
        <div className="bg-white" style={{ border: '1px solid #E8E8E8' }}>
          <div className="grid grid-cols-5 px-4 py-2.5" style={{ borderBottom: '1px solid #E8E8E8' }}>
            {['版本号', '变更说明', '操作人', '发布时间', '状态'].map((h) => (
              <span key={h} className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{h}</span>
            ))}
          </div>
          {[
            { version: 'v2.4.0', desc: '新增双向扫描支持', operator: 'zhang.wei', time: '2024-07-08 10:00', status: '当前版本', statusColor: '#0A6B3A' },
            { version: 'v2.3.1', desc: '修复银行卡号误报', operator: 'li.ming', time: '2024-06-20 14:30', status: '历史版本', statusColor: '#999999' },
            { version: 'v2.3.0', desc: '增加邮箱地址检测规则', operator: 'zhang.wei', time: '2024-06-01 09:00', status: '历史版本', statusColor: '#999999' },
          ].map((v, i, arr) => (
            <div key={v.version} className="grid grid-cols-5 items-center px-4 py-3" style={{ borderBottom: i < arr.length - 1 ? '1px solid #E8E8E8' : 'none' }}>
              <span className="font-mono text-[10px] font-semibold text-[#1A1A1A]">{v.version}</span>
              <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{v.desc}</span>
              <span className="font-mono text-[10px] font-medium text-[#999999]">{v.operator}</span>
              <span className="font-mono text-[10px] font-medium text-[#999999]">{v.time}</span>
              <div><StatusTag label={v.status} color={v.statusColor} /></div>
            </div>
          ))}
          <TablePagination current={currentPage} total={15} pageSize={10} onChange={setCurrentPage} />
        </div>
      )}

      {activeTab === '拦截记录' && (
        <div className="bg-white" style={{ border: '1px solid #E8E8E8' }}>
          <div className="grid grid-cols-6 px-4 py-2.5" style={{ borderBottom: '1px solid #E8E8E8' }}>
            {['时间', '用户', '触发规则', '级别', '操作', '结果'].map((h) => (
              <span key={h} className="font-mono text-[9px] font-semibold tracking-[0.5px] text-[#999999]">{h}</span>
            ))}
          </div>
          {[
            { time: '2024-07-10 14:32', user: 'wang.fang', rule: '身份证号码检测', severity: 'critical', action: '发送消息', result: '已拦截', resultColor: '#CF1322' },
            { time: '2024-07-10 13:15', user: 'chen.jing', rule: '银行卡号检测', severity: 'high', action: '文件上传', result: '已拦截', resultColor: '#CF1322' },
            { time: '2024-07-10 11:08', user: 'li.ming', rule: '手机号码检测', severity: 'medium', action: '发送消息', result: '已脱敏', resultColor: '#D48700' },
          ].map((r, i, arr) => (
            <div key={`${r.time}-${r.user}`} className="grid grid-cols-6 items-center px-4 py-3" style={{ borderBottom: i < arr.length - 1 ? '1px solid #E8E8E8' : 'none' }}>
              <span className="font-mono text-[10px] font-medium text-[#999999]">{r.time}</span>
              <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{r.user}</span>
              <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{r.rule}</span>
              <div><StatusTag label={severityMap[r.severity].label} color={severityMap[r.severity].color} /></div>
              <span className="font-mono text-[10px] font-medium text-[#999999]">{r.action}</span>
              <div><StatusTag label={r.result} color={r.resultColor} /></div>
            </div>
          ))}
          <TablePagination current={currentPage} total={156} pageSize={10} onChange={setCurrentPage} />
        </div>
      )}
    </div>
  )
}
