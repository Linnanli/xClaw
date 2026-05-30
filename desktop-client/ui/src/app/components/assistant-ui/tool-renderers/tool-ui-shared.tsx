/**
 * 工具渲染器共享基元 — 参考 assistant-ui with-opencode 示例
 *
 * 设计原则：
 * - 工具默认渲染为单行 inline 文本（图标 + 工具名 + 摘要）
 * - 有丰富输出时才提供 collapsible 展开
 * - text-sm text-muted-foreground 基调，tool name 用 font-medium
 */

import { type ReactNode, useState } from "react";
import {
  AlertCircleIcon,
  CheckIcon,
  ChevronRightIcon,
  LoaderIcon,
  XCircleIcon,
} from "lucide-react";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/app/components/ui/collapsible";
import { cn } from "@/app/components/ui/utils";

export type ToolCallStatusLike =
  | { type: string; reason?: string }
  | undefined;

export const truncate = (value: string, max = 80): string =>
  value.length > max ? `${value.slice(0, max - 3)}…` : value;

export const str = (value: unknown): string =>
  typeof value === "string" ? value : "";

export const basename = (filepath: string): string => {
  const parts = filepath.split("/").filter(Boolean);
  return parts[parts.length - 1] ?? filepath;
};

export const shortenPath = (filepath: string, depth = 2): string => {
  const parts = filepath.split("/").filter(Boolean);
  if (parts.length <= depth) return filepath;
  return parts.slice(-depth).join("/");
};

export const isCancelledToolStatus = (status?: ToolCallStatusLike) =>
  status?.type === "incomplete" && status.reason === "cancelled";

export function ToolStatusIcon({
  status,
  completeIcon,
}: {
  status?: ToolCallStatusLike;
  completeIcon?: ReactNode;
}) {
  const statusType = status?.type ?? "complete";
  const isCancelled = isCancelledToolStatus(status);

  if (statusType === "running") {
    return <LoaderIcon className="size-3 shrink-0 animate-spin" />;
  }

  if (statusType === "requires-action") {
    return <AlertCircleIcon className="size-3 shrink-0 text-amber-600" />;
  }

  if (statusType === "incomplete") {
    return (
      <XCircleIcon
        className={cn("size-3 shrink-0", !isCancelled && "text-destructive")}
      />
    );
  }

  return <>{completeIcon ?? <CheckIcon className="size-3 shrink-0" />}</>;
}

/** 纯 inline 单行工具壳 — 不带折叠 */
export function ToolCallShell({
  toolName,
  status,
  children,
}: {
  toolName: string;
  status?: ToolCallStatusLike;
  children?: ReactNode;
}) {
  const isCancelled = isCancelledToolStatus(status);

  return (
    <div className="flex items-center gap-2 py-0.5 text-muted-foreground text-sm">
      <ToolStatusIcon status={status} />
      <span
        className={cn(
          "flex items-center gap-1.5 truncate",
          isCancelled && "line-through opacity-50",
        )}
      >
        <span className="font-medium">{toolName}</span>
        {children}
      </span>
    </div>
  );
}

/** 带折叠的工具壳 — inline trigger + collapsible content + optional actions bar */
export function CollapsibleToolShell({
  toolName,
  status,
  summary,
  actions,
  children,
}: {
  toolName: string;
  status?: ToolCallStatusLike;
  summary?: ReactNode;
  actions?: ReactNode;
  children?: ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const isCancelled = isCancelledToolStatus(status);

  return (
    <Collapsible open={open} onOpenChange={setOpen}>
      <CollapsibleTrigger asChild>
        <div className="flex cursor-pointer select-none items-center gap-2 py-0.5 text-sm text-muted-foreground">
          <ToolStatusIcon status={status} />
          <span
            className={cn(
              "font-medium",
              isCancelled && "line-through opacity-50",
            )}
          >
            {toolName}
          </span>
          {summary}
          <div className="min-w-4 flex-1 self-center border-b-[0.5px] border-dotted border-muted-foreground/20" />
          <ChevronRightIcon
            className={cn(
              "size-3 shrink-0 transition-transform",
              open && "rotate-90",
            )}
          />
        </div>
      </CollapsibleTrigger>
      <CollapsibleContent>
        <div className="ml-5 mt-1">
          {children}
          {actions && (
            <div className="mt-2 flex items-center gap-2">{actions}</div>
          )}
        </div>
      </CollapsibleContent>
    </Collapsible>
  );
}
