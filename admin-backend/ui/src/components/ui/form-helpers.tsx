/**
 * 通用表单辅助组件
 *
 * FormLabel: 表单标签（支持必填标记）
 * FormHint: 表单提示文字
 * StatusMessage: 操作结果提示条
 */

export function FormLabel({ children, required }: { children: React.ReactNode; required?: boolean }) {
  return (
    <label className="font-mono text-[10px] font-semibold text-[#1A1A1A]">
      {children}
      {required && <span className="ml-0.5 text-[#CF1322]">*</span>}
    </label>
  )
}

export function FormHint({ children }: { children: React.ReactNode }) {
  return <span className="font-mono text-[9px] font-medium text-[#999999]">{children}</span>
}

export function StatusMessage({ type, message }: { type: 'success' | 'error'; message: string }) {
  const color = type === 'success' ? '#0A6B3A' : '#CF1322'
  return (
    <div className="px-6 py-2" style={{ backgroundColor: `${color}08`, borderBottom: '1px solid #E8E8E8' }}>
      <span className="font-mono text-[10px] font-medium" style={{ color }}>{message}</span>
    </div>
  )
}
