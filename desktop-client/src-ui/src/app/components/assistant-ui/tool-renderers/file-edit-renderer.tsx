/**
 * FileEditRenderer — CodeEditTool 输出的 inline diff 渲染器。
 *
 * 解析 code_edit 工具返回的 JSON，显示：
 * - 文件路径 + 替换计数
 * - diff 预览（红色删除行 / 绿色新增行）
 * - 可折叠（默认折叠，点击展开完整 diff）
 */
import { makeAssistantToolUI } from "@assistant-ui/react";
import { useState } from "react";
import {
  FileEditIcon,
  CheckCircle2Icon,
  ChevronDownIcon,
  ChevronRightIcon,
} from "lucide-react";
import { cn } from "@/app/components/ui/utils";

interface CodeEditArgs {
  file_path: string;
  old_string: string;
  new_string: string;
  expected_count?: number;
}

interface CodeEditResult {
  path: string;
  replaced_count: number;
  diff_preview: string;
  success: boolean;
}

function DiffView({ diff }: { diff: string }) {
  const lines = diff.split("\n");

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

export const FileEditToolUI = makeAssistantToolUI<CodeEditArgs, string>({
  toolName: "code_edit",
  render: ({ args, result, status }) => {
    const [expanded, setExpanded] = useState(false);

    let parsed: CodeEditResult | null = null;
    if (status.type === "complete" && result) {
      try {
        parsed = JSON.parse(result) as CodeEditResult;
      } catch {
        // result is plain text
      }
    }

    const filePath = parsed?.path ?? args?.file_path ?? "unknown";
    const fileName = filePath.split("/").pop() ?? filePath;

    return (
      <div className="my-1 overflow-hidden rounded-md border border-border/50 bg-muted/30 text-sm">
        {/* Header */}
        <button
          type="button"
          onClick={() => setExpanded((p) => !p)}
          className="flex w-full items-center gap-2 px-3 py-2 text-left hover:bg-muted/50"
        >
          {status.type === "running" ? (
            <span className="size-2 animate-pulse rounded-full bg-blue-400" />
          ) : parsed?.success ? (
            <CheckCircle2Icon className="size-4 text-green-500" />
          ) : (
            <FileEditIcon className="size-4 text-muted-foreground" />
          )}
          <span className="font-medium text-foreground">{fileName}</span>
          {parsed && (
            <span className="text-xs text-muted-foreground">
              {parsed.replaced_count} replacement{parsed.replaced_count !== 1 ? "s" : ""}
            </span>
          )}
          <span className="ml-auto">
            {expanded ? (
              <ChevronDownIcon className="size-4 text-muted-foreground" />
            ) : (
              <ChevronRightIcon className="size-4 text-muted-foreground" />
            )}
          </span>
        </button>

        {/* Diff body */}
        {expanded && parsed?.diff_preview && (
          <div className="border-t border-border/50 bg-[#1e1e2e]">
            <DiffView diff={parsed.diff_preview} />
          </div>
        )}

        {/* Running indicator */}
        {status.type === "running" && (
          <div className="px-3 py-1 text-xs text-muted-foreground">
            Editing {args?.file_path ?? "file"}…
          </div>
        )}

        {/* Error state */}
        {status.type === "complete" && result && !parsed && (
          <div className="border-t border-border/50 px-3 py-2 text-xs text-red-400">
            {result}
          </div>
        )}
      </div>
    );
  },
});
