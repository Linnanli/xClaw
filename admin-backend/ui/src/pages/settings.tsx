import { RotateCcw, Save, Loader2 } from 'lucide-react'
import { useState, useEffect, useCallback } from 'react'
import { api } from '@/lib/api'
import { ToggleSwitch } from '@/components/ui/toggle-switch'

/* ── 类型 ── */

interface SystemConfig {
  dlp_enabled: boolean
  dlp_scan_timeout_ms: number
  dlp_fail_open: boolean
  audit_enabled: boolean
  audit_retention_days: number
  client_heartbeat_interval_s: number
  policy_sync_interval_s: number
  policy_auto_push: boolean
}

const DEFAULT_CONFIG: SystemConfig = {
  dlp_enabled: true,
  dlp_scan_timeout_ms: 5000,
  dlp_fail_open: false,
  audit_enabled: true,
  audit_retention_days: 90,
  client_heartbeat_interval_s: 30,
  policy_sync_interval_s: 300,
  policy_auto_push: true,
}

/* ── Tab 定义 ── */

const tabs = [
  { label: 'DLP 防泄漏', badge: false },
  { label: '审计日志', badge: false },
  { label: '客户端', badge: false },
  { label: '策略同步', badge: false },
  { label: '告警通知', badge: true },
  { label: '安全策略', badge: true },
  { label: '水印配置', badge: true },
]

/* ── 设置项行 ── */

function SettingRow({ label, value, warning }: { label: string; value: React.ReactNode; warning?: string }) {
  return (
    <div className="flex items-center justify-between">
      <div className="flex flex-col gap-1">
        <span className="font-mono text-[10px] font-medium text-[#1A1A1A]">{label}</span>
        {warning && <span className="font-mono text-[9px] font-medium text-[#CF1322]">{warning}</span>}
      </div>
      {value}
    </div>
  )
}

function SettingValue({ text }: { text: string }) {
  return <span className="font-mono text-[10px] font-semibold text-[#1A1A1A]">{text}</span>
}

function SettingCard({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="flex flex-col gap-4 bg-white p-5" style={{ border: '1px solid #E8E8E8' }}>
      <span className="text-sm font-semibold text-[#1A1A1A]" style={{ fontFamily: 'Space Grotesk, sans-serif' }}>{title}</span>
      {children}
    </div>
  )
}

/* ── 主组件 ── */

export default function SettingsPage() {
  const [activeTab, setActiveTab] = useState('DLP 防泄漏')
  const [config, setConfig] = useState<SystemConfig>(DEFAULT_CONFIG)
  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState(false)

  const loadSettings = useCallback(async () => {
    setLoading(true)
    try {
      const res = await api.get('/settings')
      setConfig({ ...DEFAULT_CONFIG, ...res.data })
    } catch (err) {
      console.error('加载系统配置失败', err)
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => { loadSettings() }, [loadSettings])

  const handleSave = useCallback(async () => {
    setSaving(true)
    try {
      await api.put('/settings', config)
    } catch (err) {
      console.error('保存配置失败', err)
    } finally {
      setSaving(false)
    }
  }, [config])

  const handleReset = useCallback(() => {
    setConfig(DEFAULT_CONFIG)
  }, [])

  const updateField = useCallback(<K extends keyof SystemConfig>(key: K, value: SystemConfig[K]) => {
    setConfig((prev) => ({ ...prev, [key]: value }))
  }, [])

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
          <button
            className="flex items-center gap-2 border border-[#E8E8E8] bg-white px-4 py-2.5 font-mono text-[9px] font-semibold text-[#1A1A1A]"
            onClick={handleReset}
          >
            <RotateCcw className="h-3 w-3 text-[#6a6a6a]" />
            恢复默认
          </button>
          <button
            className="flex items-center gap-2 bg-[#0A6B3A] px-4 py-2.5 font-mono text-[9px] font-semibold text-white"
            onClick={handleSave}
          >
            {saving ? <Loader2 className="h-3 w-3 animate-spin" /> : <Save className="h-3 w-3" />}
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
              <span className="px-1.5 py-0.5 font-mono text-[8px] font-semibold" style={{ color: '#0A6B3A', backgroundColor: 'rgba(10,107,58,0.13)', border: '1px solid #0A6B3A' }}>
                新增
              </span>
            )}
          </button>
        ))}
      </div>

      {/* 加载状态 */}
      {loading ? (
        <div className="flex items-center justify-center py-10">
          <Loader2 className="h-5 w-5 animate-spin text-[#999999]" />
        </div>
      ) : (
        <>
          {/* Tab 内容区 */}
          {activeTab === 'DLP 防泄漏' && (
            <SettingCard title="DLP 数据防泄漏">
              <SettingRow label="启用 DLP 扫描" value={<ToggleSwitch on={config.dlp_enabled} onChange={() => updateField('dlp_enabled', !config.dlp_enabled)} />} />
              <SettingRow label="扫描超时" value={<SettingValue text={`${config.dlp_scan_timeout_ms}ms`} />} />
              <SettingRow label="故障开放模式" value={<ToggleSwitch on={config.dlp_fail_open} onChange={() => updateField('dlp_fail_open', !config.dlp_fail_open)} />} warning="⚠ 启用后 DLP 故障时将放行请求，存在安全风险" />
            </SettingCard>
          )}

          {activeTab === '审计日志' && (
            <SettingCard title="审计日志">
              <SettingRow label="启用审计" value={<ToggleSwitch on={config.audit_enabled} onChange={() => updateField('audit_enabled', !config.audit_enabled)} />} />
              <SettingRow label="保留天数" value={<SettingValue text={`${config.audit_retention_days} 天`} />} />
            </SettingCard>
          )}

          {activeTab === '客户端' && (
            <SettingCard title="客户端配置">
              <SettingRow label="心跳间隔" value={<SettingValue text={`${config.client_heartbeat_interval_s} 秒`} />} />
            </SettingCard>
          )}

          {activeTab === '策略同步' && (
            <SettingCard title="策略同步">
              <SettingRow label="自动推送" value={<ToggleSwitch on={config.policy_auto_push} onChange={() => updateField('policy_auto_push', !config.policy_auto_push)} />} />
              <SettingRow label="同步间隔" value={<SettingValue text={`${config.policy_sync_interval_s} 秒`} />} />
            </SettingCard>
          )}

          {activeTab === '告警通知' && (
            <SettingCard title="告警通知渠道">
              {[
                { name: '邮件', on: true },
                { name: '企微', on: true },
                { name: '钉钉', on: false },
                { name: '飞书', on: false },
              ].map((ch) => (
                <SettingRow key={ch.name} label={ch.name} value={<ToggleSwitch on={ch.on} />} />
              ))}
            </SettingCard>
          )}

          {activeTab === '安全策略' && (
            <SettingCard title="安全策略">
              <SettingRow label="密码最小长度" value={<SettingValue text="8 位" />} />
              <SettingRow label="会话超时" value={<SettingValue text="480 分钟" />} />
              <SettingRow label="登录失败锁定" value={<SettingValue text="5 次 / 15 分钟" />} />
              <SettingRow label="强制 MFA" value={<ToggleSwitch on={false} />} />
            </SettingCard>
          )}

          {activeTab === '水印配置' && (
            <SettingCard title="水印配置">
              <SettingRow label="启用水印" value={<ToggleSwitch on />} />
              <SettingRow label="水印类型" value={<SettingValue text="文本水印" />} />
              <SettingRow label="水印透明度" value={<SettingValue text="15%" />} />
              <SettingRow label="包含用户信息" value={<ToggleSwitch on />} />
            </SettingCard>
          )}
        </>
      )}
    </div>
  )
}
