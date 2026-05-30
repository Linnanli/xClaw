/**
 * ConfirmDialog — 通用确认弹窗
 *
 * 设计参考：client-design.pen Node 2HGWJ / ConfirmModal
 * 基于 shadcn/ui AlertDialog 实现，支持自定义图标、标题、描述和按钮文案。
 */

import { type ReactNode, type FC } from 'react';
import { Trash2 } from 'lucide-react';
import {
  AlertDialog,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '../ui/alert-dialog';
import { Button } from '../ui/button';
import { Separator } from '../ui/separator';
import { cn } from '../ui/utils';

// ── 类型定义 ──────────────────────────────────────────────────────

type ConfirmVariant = 'danger' | 'warning' | 'default';

interface ConfirmDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** 弹窗顶部图标，默认为 Trash2（danger 变体） */
  icon?: ReactNode;
  variant?: ConfirmVariant;
  title: string;
  description: string;
  cancelText?: string;
  confirmText?: string;
  onConfirm: () => void;
  loading?: boolean;
}

// ── 变体样式映射 ──────────────────────────────────────────────────

const VARIANT_STYLES: Record<ConfirmVariant, { iconBg: string; iconColor: string; confirmBtn: string }> = {
  danger:  { iconBg: 'bg-[#FFF0F0]', iconColor: 'text-[#D94040]', confirmBtn: 'bg-[#D94040] hover:bg-[#C03030] text-white' },
  warning: { iconBg: 'bg-amber-50',  iconColor: 'text-amber-500',  confirmBtn: 'bg-amber-500 hover:bg-amber-600 text-white' },
  default: { iconBg: 'bg-primary/10', iconColor: 'text-primary',   confirmBtn: 'bg-primary hover:bg-primary/90 text-primary-foreground' },
};

// ── 组件 ──────────────────────────────────────────────────────────

export const ConfirmDialog: FC<ConfirmDialogProps> = ({
  open,
  onOpenChange,
  icon,
  variant = 'danger',
  title,
  description,
  cancelText = '取消',
  confirmText = '确认',
  onConfirm,
  loading = false,
}) => {
  const styles = VARIANT_STYLES[variant];
  const resolvedIcon = icon ?? <Trash2 className={cn('size-[22px]', styles.iconColor)} />;

  return (
    <AlertDialog open={open} onOpenChange={onOpenChange}>
      <AlertDialogContent className="w-[440px] max-w-[95vw] gap-0 overflow-hidden rounded-2xl p-0 shadow-[0_24px_80px_#00000020,0_8px_20px_#0000000a]">
        {/* 顶部：图标 + 标题 + 描述 */}
        <AlertDialogHeader className="flex flex-col items-center gap-4 px-6 pb-6 pt-6 text-center">
          <div className={cn('flex size-12 items-center justify-center rounded-xl', styles.iconBg)}>
            {resolvedIcon}
          </div>
          <AlertDialogTitle className="text-[18px] font-bold leading-snug tracking-[-0.2px] text-[#1A1918]">
            {title}
          </AlertDialogTitle>
          <AlertDialogDescription className="text-[13px] leading-relaxed text-[#6D6C6A]">
            {description}
          </AlertDialogDescription>
        </AlertDialogHeader>

        <Separator className="bg-[#E5E4E1]" />

        {/* 底部按钮 */}
        <AlertDialogFooter className="flex flex-row items-center justify-end gap-3 px-6 py-4">
          <Button
            variant="outline"
            className="rounded-[10px] border-[#E5E4E1] px-5 py-2.5 text-[14px] font-medium text-[#4A4947] hover:bg-[#F5F4F1]"
            onClick={() => onOpenChange(false)}
            disabled={loading}
          >
            {cancelText}
          </Button>
          <Button
            className={cn('gap-1.5 rounded-[10px] px-5 py-2.5 text-[14px] font-semibold', styles.confirmBtn)}
            onClick={onConfirm}
            disabled={loading}
          >
            {variant === 'danger' && <Trash2 className="size-3.5" />}
            {confirmText}
          </Button>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
};
