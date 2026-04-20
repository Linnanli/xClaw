/**
 * ContextToolGroup — 将连续的上下文采集工具步骤合并为一行带计数的指示器。
 *
 * read_file × 3、grep_search × 2、list_files × 1
 *   → "📚 已收集上下文（6 项）  read: 3 | grep: 2 | list: 1"
 *
 * 设计决策：只作用于流式时间线的 ToolStep 指示器，
 * 不影响 assistant-ui 的完整 ToolUI Parts 渲染。
 */

import { memo, useState } from "react";
import { CheckIcon, ChevronRightIcon, LoaderIcon, BookOpenIcon } from "lucide-react";
import { cn } from "@/app/components/ui/utils";
import { ToolStepIndicator } from "./tool-step-indicator";
import type { ToolStep } from "@/app/runtime/TauriRuntimeProvider";

/** 被视为"上下文采集"的工具名集合 */
const CONTEXT_TOOLS = new Set([
  "read_file",
  "glob_search",
  "grep_search",
  "list_files",
  "search_files",
]);

export function isContextTool(toolName: string): boolean {
  return CONTEXT_TOOLS.has(toolName);
}

/**
 * 将扁平的 ToolStep[] 分段：
 * 连续的 context tool 合并为 { type: "group", steps }，
 * 其余保持为 { type: "single", step }。
 */
export type StepSegment =
  | { type: "single"; step: ToolStep }
  | { type: "group"; steps: ToolStep[] };

export function groupToolSteps(steps: ToolStep[]): StepSegment[] {
  const segments: StepSegment[] = [];
  let contextBuf: ToolStep[] = [];

  function flushContext() {
    if (contextBuf.length === 0) return;
    if (contextBuf.length === 1) {
      segments.push({ type: "single", step: contextBuf[0] });
    } else {
      segments.push({ type: "group", steps: [...contextBuf] });
    }
    contextBuf = [];
  }

  for (const step of steps) {
    if (isContextTool(step.toolName)) {
      contextBuf.push(step);
    } else {
      flushContext();
      segments.push({ type: "single", step });
    }
  }
  flushContext();
  return segments;
}

/** 统计每种工具的出现次数 */
function countByTool(steps: ToolStep[]): Map<string, number> {
  const map = new Map<string, number>();
  for (const s of steps) {
    map.set(s.toolName, (map.get(s.toolName) ?? 0) + 1);
  }
  return map;
}

const SHORT_NAME: Record<string, string> = {
  read_file: "read",
  glob_search: "glob",
  grep_search: "grep",
  list_files: "list",
  search_files: "search",
};

interface ContextToolGroupProps {
  steps: ToolStep[];
}

function ContextToolGroupImpl({ steps }: ContextToolGroupProps) {
  const [expanded, setExpanded] = useState(false);
  const counts = countByTool(steps);
  const total = steps.length;
  const hasRunning = steps.some((s) => s.status === "running");
  const hasError = steps.some((s) => s.status === "error");

  const summaryParts = Array.from(counts.entries()).map(
    ([tool, count]) => `${SHORT_NAME[tool] ?? tool}: ${count}`,
  );

  return (
    <div className="flex flex-col gap-0">
      <button
        type="button"
        onClick={() => setExpanded((v) => !v)}
        className={cn(
          "flex items-center gap-2 py-0.5 text-xs transition-colors hover:text-foreground",
          hasError ? "text-destructive/80" : "text-muted-foreground",
        )}
      >
        {hasRunning ? (
          <LoaderIcon className="size-3 shrink-0 animate-spin" />
        ) : hasError ? (
          <BookOpenIcon className="size-3 shrink-0" />
        ) : (
          <CheckIcon className="size-3 shrink-0 text-emerald-500" />
        )}
        <span>
          已收集上下文（{total} 项）
          <span className="ml-2 opacity-50">{summaryParts.join(" · ")}</span>
        </span>
        <ChevronRightIcon
          className={cn(
            "size-3 shrink-0 transition-transform",
            expanded && "rotate-90",
          )}
        />
      </button>

      {expanded && (
        <div className="ml-5 flex flex-col gap-0">
          {steps.map((step, idx) => (
            <ToolStepIndicator
              key={`${step.toolName}-${step.startedAt}-${idx}`}
              toolName={step.toolName}
              status={step.status}
              error={step.error}
            />
          ))}
        </div>
      )}
    </div>
  );
}

export const ContextToolGroup = memo(ContextToolGroupImpl);
ContextToolGroup.displayName = "ContextToolGroup";
