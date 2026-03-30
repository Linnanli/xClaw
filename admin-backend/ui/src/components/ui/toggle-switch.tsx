/**
 * 通用开关组件
 *
 * 设计规范：32x20px，开启 #0A6B3A，关闭 #D9D9D9，白色圆形滑块
 */
export function ToggleSwitch({ on, onChange }: { on: boolean; onChange?: () => void }) {
  return (
    <div
      className="relative h-5 w-9 cursor-pointer"
      style={{ backgroundColor: on ? '#0A6B3A' : '#D9D9D9', borderRadius: 10 }}
      onClick={onChange}
      data-testid={`toggle-${on ? 'on' : 'off'}`}
    >
      <div
        className="absolute top-0.5 h-4 w-4 bg-white"
        style={{ borderRadius: 8, left: on ? 18 : 2, transition: 'left 0.2s' }}
      />
    </div>
  )
}
