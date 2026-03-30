import { RotateCcw, Save } from 'lucide-react'
import { useState } from 'react'

/* ── Mock 数据 ── */

const tabs = [
  { label: 'DLP 防泄漏', badge: false },
  { label: '审计日志', badge: false },
  { label: '客户端', badge: false },
  { label: '策略同步', badge: false },
  { label: '告警通知', badge: true },
  { label: '安全策略', badge: true },
  { label: '水印配置', badge: true },
]

/* ── Toggle 组件 ── */

function Toggle({ on }: { on: boolean }) {
  return (
    <div
      className="relative h-5 w-9 cursor-pointer"
      style={{ backgroundColor: on ? '#0A6B3A' : '#D9D9D9', borderRadius: 10 }}
      data-testid={`toggle-${on ? 'on' : 'off'}`}
    >
      <div
        className="absolute top-0.5 h-4 w-4 bg-white"
        style={{ borderRadius: 8, left: on ? 18 : 2, transition: 'left 0.2s' }}
      />
    </div>
  )
}

/* ── 组件 ── */

export default function SettingsPage() {
  const [activeTab, setActiveTab] = useState('DLP 防泄漏')

  return (
    <div className="flex flex-col gap-6">

      {/* 标题区 */}
      <div className="flex items-end justify-between">
        <div className="flex flex-col gap-2">
          <h1 className="text-[22px] font-bold tracking-tight text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif', letterSpacing: '-1px' }}>
            系统设置
          </h1>
          <p className="font-mono text-xs text-[#999999]">DLP、审计、客户端、告警通知、安全策略与水印配置</p>
        </div>
        <div className="flex gap-2">
          <button className="flex items-center gap-2 border border-[#E8E8E8] bg-white px-4 py-2.5 font-mono text-[9px] font-semibold text-[#1A1A1A]">
            <RotateCcw className="h-3 w-3 text-[#6a6a6a]" />
            恢复默认
          </button>
          <button className="flex items-center gap-2 bg-[#0A6B3A] px-4 py-2.5 font-mono text-[9px] font-semibold text-white">
            <Save className="h-3 w-3" />
            保存配置
          </button>
        </div>
      </div>

      {/* Tab 栏 */}
      <div className="flex gap-0" style={{ borderBottom: '1px solid #E8E8E8' }}>
        {tabs.map((tab) => (
          <button
            key={tab.label}
            onClick={() => setActiveTab(tab.label)}
            className="flex items-center gap-1.5 px-5 py-3 font-mono text-[10px] font-semibold"
            style={{
              color: activeTab === tab.label ? '#0A6B3A' : '#999999',
              borderBottom: activeTab === tab.label ? '2px solid #0A6B3A' : '2px solid transparent',
            }}
          >
            {tab.label}
            {tab.badge && (
              <span
                className="px-1.5 py-0.5 font-mono text-[8px] font-semibold"
                style={{ color: '#0A6B3A', backgroundColor: 'rgba(10,107,58,0.13)', border: '1px solid #0A6B3A' }}
              >
                新增
              </span>
            )}
          </button>
        ))}
      </div>

      {/* 2 列布局 */}
      <div className="grid grid-cols-2 gap-4">
        {/* 左列 */}
        <div className="flex flex-col gap-4">
          {/* DLP 数据防泄漏 */}
          <div className="flex flex-col gap-4 bg-white p-5" style={{ border: '1px solid #E8E8E8' }}>
            <span className="text-sm font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>
              DLP 数据防泄漏
            </span>
            <div className="flex items-center justify-between">
              <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">启用 DLP 扫描</span>
              <Toggle on />
            </div>
            <div className="flex items-center justify-between">
              <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">双向扫描</span>
              <Toggle on />
            </div>
            <div className="flex items-center justify-between">
              <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">扫描超时</span>
              <span className="font-mono text-[10px] font-semibold text-[#1A1A1A]">5000ms</span>
            </div>
            <div className="flex items-center justify-between">
              <div className="flex flex-col gap-1">
                <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">故障开放模式</span>
                <span className="font-mono text-[9px] font-medium text-[#CF1322]">⚠ 启用后 DLP 故障时将放行请求，存在安全风险</span>
              </div>
              <Toggle on={false} />
            </div>
          </div>

          {/* 审计日志 */}
          <div className="flex flex-col gap-4 bg-white p-5" style={{ border: '1px solid #E8E8E8' }}>
            <span className="text-sm font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>
              审计日志
            </span>
            <div className="flex items-center justify-between">
              <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">启用审计</span>
              <Toggle on />
            </div>
            <div className="flex items-center justify-between">
              <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">保留天数</span>
              <span className="font-mono text-[10px] font-semibold text-[#1A1A1A]">90 天</span>
            </div>
          </div>
        </div>

        {/* 右列 */}
        <div className="flex flex-col gap-4">
          {/* 安全策略 */}
          <div className="flex flex-col gap-4 bg-white p-5" style={{ border: '1px solid #E8E8E8' }}>
            <span className="text-sm font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>
              安全策略
            </span>
            <div className="flex items-center justify-between">
              <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">密码最小长度</span>
              <span className="font-mono text-[10px] font-semibold text-[#1A1A1A]">8 位</span>
            </div>
            <div className="flex items-center justify-between">
              <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">会话超时</span>
              <span className="font-mono text-[10px] font-semibold text-[#1A1A1A]">480 分钟</span>
            </div>
            <div className="flex items-center justify-between">
              <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">登录失败锁定</span>
              <span className="font-mono text-[10px] font-semibold text-[#1A1A1A]">5 次 / 15 分钟</span>
            </div>
          </div>

          {/* 告警通知渠道 */}
          <div className="flex flex-col gap-4 bg-white p-5" style={{ border: '1px solid #E8E8E8' }}>
            <span className="text-sm font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>
              告警通知渠道
            </span>
            {[
              { name: '邮件', on: true },
              { name: '企微', on: true },
              { name: '钉钉', on: false },
              { name: '飞书', on: false },
            ].map((ch) => (
              <div key={ch.name} className="flex items-center justify-between">
                <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{ch.name}</span>
                <Toggle on={ch.on} />
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  )
}
