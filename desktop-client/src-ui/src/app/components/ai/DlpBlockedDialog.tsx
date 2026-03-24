/**
 * DlpBlockedDialog - DLP 消息阻止对话框
 *
 * 使用 shadcn/ui AlertDialog，当用户消息被 DLP 策略阻止时弹出。
 * 提供明确的阻止原因和操作指引。
 *
 * 设计规范：
 * - 使用 destructive 色调强调安全性
 * - ShieldX 图标 + 明确的阻止原因
 * - 提供"知道了"确认按钮
 * - 可选"编辑消息"按钮（回到输入框修改）
 */

import { ShieldX } from 'lucide-react';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '../ui/alert-dialog';
import { Badge } from '../ui/badge';
import type { SanitizationStats } from '../../hooks/useDlpScan';

export interface DlpBlockedDialogProps {
  /** 是否打开 */
  open: boolean;
  /** 关闭回调 */
  onClose: () => void;
  /** 编辑消息回调（可选，回到输入框修改） */
  onEdit?: () => void;
  /** 阻止原因 */
  blockReason?: string;
  /** 脱敏统计 */
  stats?: SanitizationStats;
}

export function DlpBlockedDialog({
  open,
  onClose,
  onEdit,
  blockReason,
  stats,
}: DlpBlockedDialogProps) {
  return (
    <AlertDialog open={open} onOpenChange={(v) => !v && onClose()}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle className="flex items-center gap-2 text-destructive">
            <ShieldX className="size-5" />
            消息发送被阻止
          </AlertDialogTitle>
          <AlertDialogDescription asChild>
            <div className="space-y-3">
              <p>
                您的消息包含高危敏感信息，已被 DLP 安全策略阻止发送。
              </p>

              {blockReason && (
                <div className="rounded-md bg-destructive/10 px-3 py-2 text-sm text-destructive">
                  <span className="font-medium">阻止原因：</span>
                  {blockReason}
                </div>
              )}

              {stats && (stats.blocked_count > 0 || stats.total_matches > 0) && (
                <div className="flex flex-wrap gap-2">
                  {stats.blocked_count > 0 && (
                    <Badge variant="destructive">
                      {stats.blocked_count} 处高危信息
                    </Badge>
                  )}
                  {stats.redacted_count > 0 && (
                    <Badge variant="secondary">
                      {stats.redacted_count} 处已脱敏
                    </Badge>
                  )}
                </div>
              )}

              <p className="text-xs text-muted-foreground">
                请移除敏感信息后重新发送。如有疑问，请联系管理员。
              </p>
            </div>
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          {onEdit && (
            <AlertDialogCancel onClick={onEdit}>
              编辑消息
            </AlertDialogCancel>
          )}
          <AlertDialogAction
            className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
            onClick={onClose}
          >
            知道了
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
