/**
 * GitDiffRenderer — git_diff 工具的 inline diff 渲染器。
 *
 * 解析 git_diff 工具返回的文本输出，显示：
 * - diff --stat 概要（文件变更统计）
 * - diff --patch 内容（红色删除行 / 绿色新增行）
 * - 可折叠（默认折叠，点击展开完整 diff）
 */
import { makeAssistantToolUI } from "@assistant-ui/react";
import { useState } from "react";
import {
  GitBranchIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  LoaderIcon,
} from "lucide-react";

interface GitDiffArgs {
  staged?: boolean;
  file_path?: string;
  path?: string;
}

function DiffView({ text }: { text: string }) {
  const lines = text.split("\n");

  return (
    <pre className="overflow-x-auto text-xs leading-relaxed">
      {lines.map((line, i) => {
        let className = "px-2";
        if (line.startsWith("+") && !line.startsWith("+++")) {
          className = "bg-green-500/15 text-green-400 px-2";
        } else if (line.startsWith("-") && !line.startsWith("---")) {
          className = "bg-red-500/15 text-red-400 px-2";
        } else if (line.startsWith("@@")) {
          className = "text-blue-400 px-2";
        } else if (line.startsWith("diff --git")) {
          className = "text-yellow-400 font-semibold px-2";
        }
        return (
          <div key={i} className={className}>
            {line || "\u00A0"}
          </div>
        );
      })}
    </pre>
  );
}

export const GitDiffToolUI = makeAssistantToolUI<GitDiffArgs, string>({
  toolName: "git_diff",
  render: ({ args, result, status }) => {
    const [expanded, setExpanded] = useState(false);
    const staged = args?.staged ?? false;
    const label = staged ? "Staged changes" : "Unstaged changes";
    const fileFilter = args?.file_path;

    const isRunning = status.type === "running";
    const noChanges =
      status.type === "complete" &&
      result &&
      (result.includes("No ") && result.includes("changes"));

    return (
      <div className="my-1 overflow-hidden rounded-md border border-border/50 bg-muted/30 text-sm">
        {/* Header */}
        <button
          type="button"
          onClick={() => setExpanded((p) => !p)}
          className="flex w-full items-center gap-2 px-3 py-2 text-left hover:bg-muted/50"
        >
          {isRunning ? (
            <LoaderIcon className="h-4 w-4 animate-spin text-muted-foreground" />
          ) : expanded ? (
            <ChevronDownIcon className="h-4 w-4 text-muted-foreground" />
          ) : (
            <ChevronRightIcon className="h-4 w-4 text-muted-foreground" />
          )}
          <GitBranchIcon className="h-4 w-4 text-blue-400" />
          <span className="font-medium text-foreground">{label}</span>
          {fileFilter && (
            <span className="text-muted-foreground">— {fileFilter}</span>
          )}
          {noChanges && (
            <span className="ml-auto text-muted-foreground text-xs">
              No changes
            </span>
          )}
        </button>

        {/* Body */}
        {expanded && status.type === "complete" && result && (
          <div className="border-t border-border/50 bg-[#1e1e1e] p-2">
            <DiffView text={result} />
          </div>
        )}
      </div>
    );
  },
});
