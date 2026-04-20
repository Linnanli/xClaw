/**
 * ToolStepIndicator — Cline 风格的工具执行步骤指示器。
 *
 * 用于流式过程中展示 tool_started/tool_completed 事件的实时状态。
 * 与 ToolFallback（完整的工具调用面板）不同，此组件是轻量的单行指示器：
 *   ⏳ Running shell...   （进行中：spinner + 工具名）
 *   ✓ shell              （成功：绿色勾 + 工具名）
 *   ✗ shell — Error msg  （失败：红色叉 + 工具名 + 错误）
 */

import { memo } from "react";
import { CheckIcon, LoaderIcon, XCircleIcon } from "lucide-react";
import { cn } from "@/app/components/ui/utils";

// 工具名 → 人类友好的中文动词描述
const TOOL_VERB_MAP: Record<string, string> = {
  shell: '执行命令',
  read_file: '读取文件',
  write_file: '写入文件',
  edit_file: '编辑文件',
  search_files: '搜索文件',
  list_files: '列举文件',
  glob_search: '文件匹配',
  grep_search: '内容搜索',
  http: '请求网络',
  lsp_query: 'LSP 查询',
  plan_mode: '规划',
  session_fork: '分支会话',
  sub_agent: '子 Agent',
};

function toolVerb(toolName: string): string {
  return TOOL_VERB_MAP[toolName] ?? `运行 ${toolName}`;
}

interface ToolStepIndicatorProps {
  toolName: string;
  status: 'running' | 'complete' | 'error';
  error?: string;
}

function ToolStepIndicatorImpl({ toolName, status, error }: ToolStepIndicatorProps) {
  return (
    <div
      className={cn(
        "flex items-center gap-2 py-0.5 text-xs",
        status === 'running' && "text-muted-foreground",
        status === 'complete' && "text-muted-foreground/70",
        status === 'error' && "text-destructive/80",
      )}
    >
      {status === 'running' && (
        <LoaderIcon className="size-3 shrink-0 animate-spin" />
      )}
      {status === 'complete' && (
        <CheckIcon className="size-3 shrink-0 text-emerald-500" />
      )}
      {status === 'error' && (
        <XCircleIcon className="size-3 shrink-0" />
      )}
      <span className="truncate">
        {status === 'running' ? `${toolVerb(toolName)}...` : toolName}
        {status === 'error' && error && (
          <span className="ml-1.5 text-[10px] opacity-70">— {error}</span>
        )}
      </span>
    </div>
  );
}

export const ToolStepIndicator = memo(ToolStepIndicatorImpl);
ToolStepIndicator.displayName = "ToolStepIndicator";

export type { ToolStepIndicatorProps };
