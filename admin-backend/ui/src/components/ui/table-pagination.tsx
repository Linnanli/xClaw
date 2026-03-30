import { ChevronLeft, ChevronRight } from 'lucide-react'

/**
 * 通用表格分页组件
 *
 * 设计规范：
 * - 字体：JetBrains Mono 10px
 * - 颜色：#999999（文字）、#0A6B3A（活跃页码）、#E8E8E8（边框）
 * - 布局：左侧总数 + 右侧页码导航
 */

interface TablePaginationProps {
  /** 当前页码（从 1 开始） */
  current: number
  /** 总记录数 */
  total: number
  /** 每页条数 */
  pageSize: number
  /** 页码变更回调 */
  onChange: (page: number) => void
}

export function TablePagination({ current, total, pageSize, onChange }: TablePaginationProps) {
  const totalPages = Math.max(1, Math.ceil(total / pageSize))
  const pages = buildPageNumbers(current, totalPages)

  return (
    <div
      className="flex items-center justify-between px-4 py-3"
      style={{ borderTop: '1px solid #E8E8E8' }}
      data-testid="table-pagination"
    >
      <span className="font-mono text-[10px] font-medium text-[#999999]">
        共 {total.toLocaleString()} 条记录
      </span>

      <div className="flex items-center gap-1">
        {/* 上一页 */}
        <PageButton
          disabled={current <= 1}
          onClick={() => onChange(current - 1)}
          aria-label="上一页"
        >
          <ChevronLeft className="h-3 w-3" />
        </PageButton>

        {/* 页码 */}
        {pages.map((p, i) =>
          p === '...' ? (
            <span key={`ellipsis-${i}`} className="px-1 font-mono text-[10px] text-[#999999]">...</span>
          ) : (
            <PageButton
              key={p}
              active={p === current}
              onClick={() => onChange(p as number)}
            >
              {p}
            </PageButton>
          )
        )}

        {/* 下一页 */}
        <PageButton
          disabled={current >= totalPages}
          onClick={() => onChange(current + 1)}
          aria-label="下一页"
        >
          <ChevronRight className="h-3 w-3" />
        </PageButton>
      </div>
    </div>
  )
}

/* ── 页码按钮 ── */

function PageButton({
  active,
  disabled,
  children,
  onClick,
  ...rest
}: {
  active?: boolean
  disabled?: boolean
  children: React.ReactNode
  onClick?: () => void
} & React.ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <button
      className="flex h-7 min-w-7 items-center justify-center font-mono text-[10px] font-semibold"
      style={{
        color: active ? '#FFFFFF' : disabled ? '#CCCCCC' : '#999999',
        backgroundColor: active ? '#0A6B3A' : 'transparent',
        border: active ? 'none' : '1px solid transparent',
        cursor: disabled ? 'default' : 'pointer',
        opacity: disabled ? 0.5 : 1,
      }}
      disabled={disabled}
      onClick={onClick}
      {...rest}
    >
      {children}
    </button>
  )
}

/* ── 页码生成算法 ── */

function buildPageNumbers(current: number, total: number): (number | '...')[] {
  if (total <= 7) {
    return Array.from({ length: total }, (_, i) => i + 1)
  }

  const pages: (number | '...')[] = [1]

  if (current > 3) pages.push('...')

  const start = Math.max(2, current - 1)
  const end = Math.min(total - 1, current + 1)

  for (let i = start; i <= end; i++) pages.push(i)

  if (current < total - 2) pages.push('...')

  pages.push(total)
  return pages
}
