/**
 * GlobResultRenderer — GlobSearchTool 输出的文件树渲染器。
 *
 * 解析 glob_search 文本输出，显示匹配文件列表（带文件图标）。
 */
import { makeAssistantToolUI } from "@assistant-ui/react";
import { useState } from "react";
import {
  FolderSearchIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  FileIcon,
} from "lucide-react";

interface GlobSearchArgs {
  pattern: string;
  path?: string;
  exclude?: string;
  max_depth?: number;
  max_results?: number;
}

function parseGlobOutput(text: string): { count: number; files: string[] } {
  const lines = text.trim().split("\n");
  if (lines.length === 1 && lines[0] === "No files matched.") {
    return { count: 0, files: [] };
  }

  // First line: "Found N files:"
  const headerMatch = lines[0]?.match(/^Found (\d+) files:/);
  const count = headerMatch ? parseInt(headerMatch[1], 10) : 0;

  const files = lines
    .slice(1)
    .map((l) => l.trim())
    .filter((l) => l.length > 0);

  return { count, files };
}

export const GlobResultToolUI = makeAssistantToolUI<GlobSearchArgs, string>({
  toolName: "glob_search",
  render: ({ args, result, status }) => {
    const [expanded, setExpanded] = useState(false);

    const { count, files } = result
      ? parseGlobOutput(result)
      : { count: 0, files: [] };

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
          ) : (
            <FolderSearchIcon className="size-4 text-muted-foreground" />
          )}
          <span className="font-mono text-xs text-foreground">
            {args?.pattern ?? "glob"}
          </span>
          {status.type === "complete" && (
            <span className="text-xs text-muted-foreground">
              {count} file{count !== 1 ? "s" : ""}
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

        {/* Running indicator */}
        {status.type === "running" && (
          <div className="px-3 py-1 text-xs text-muted-foreground">
            Searching for <code>{args?.pattern}</code>…
          </div>
        )}

        {/* File list */}
        {expanded && files.length > 0 && (
          <div className="border-t border-border/50 px-3 py-1.5">
            {files.map((file, i) => (
              <div
                key={i}
                className="flex items-center gap-1.5 py-0.5 text-xs"
              >
                <FileIcon className="size-3 text-muted-foreground" />
                <span className="font-mono text-foreground">{file}</span>
              </div>
            ))}
          </div>
        )}

        {/* No results */}
        {expanded && status.type === "complete" && count === 0 && (
          <div className="border-t border-border/50 px-3 py-2 text-xs text-muted-foreground">
            No files matched.
          </div>
        )}
      </div>
    );
  },
});
