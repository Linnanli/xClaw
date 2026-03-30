/**
 * 通用彩色状态标签
 *
 * 设计规范：文字颜色 + 10% 透明度背景 + 1px 实线边框
 */
export function StatusTag({ label, color }: { label: string; color: string }) {
  return (
    <span
      className="inline-block px-2 py-0.5 font-mono text-[9px] font-semibold"
      style={{ color, backgroundColor: `${color}1A`, border: `1px solid ${color}` }}
    >
      {label}
    </span>
  )
}
