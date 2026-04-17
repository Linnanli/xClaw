/**
 * GrepResultRenderer — GrepSearchTool 输出的搜索结果渲染器。
 *
 * 显示匹配结果列表：文件名 + 行号 + 匹配行内容。
 * 支持上下文行（context before/after）。
 */
import { makeAssistantToolUI } from "@assistant-ui/react";
import { useState } from "react";
import {
  SearchIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  FileTextIcon,
} from "lucide-react";
import { cn } from "@/app/components/ui/utils";

interface GrepSearchArgs {
  pattern: string;
  path?: string;
  context_before?: number;
  context_after?: number;
  max_results?: number;
}

/** 解析 grep 文本输出为结构化结果 */
function parseGrepOutput(text: string): GrepFileGroup[] {
  const groups: GrepFileGroup[] = [];
  let current: GrepFileGroup | null = null;

  for (const line of text.split("\n")) {
    // 文件头：── path/to/file ──
    const fileMatch = line.match(/^──\s+(.+?)\s+──$/);
    if (fileMatch) {
      current = { file: fileMatch[1], matches: [] };
      groups.push(current);
      continue;
    }
    // 匹配行: "  42: some content"
    const lineMatch = line.match(/^\s*(\d+):\s(.*)$/);
    if (lineMatch && current) {
      current.matches.push({
        lineNumber: parseInt(lineMatch[1], 10),
        content: lineMatch[2],
        isMatch: true,
      });
      continue;
    }
    // 上下文行: "  40- some context"
    const ctxMatch = line.match(/^\s*(\d+)-\s(.*)$/);
    if (ctxMatch && current) {
      current.matches.push({
        lineNumber: parseInt(ctxMatch[1], 10),
        content: ctxMatch[2],
        isMatch: false,
      });
    }
  }
  return groups;
}

interface GrepLine {
  lineNumber: number;
  content: string;
  isMatch: boolean;
}

interface GrepFileGroup {
  file: string;
  matches: GrepLine[];
}

export const GrepResultToolUI = makeAssistantToolUI<GrepSearchArgs, string>({
  toolName: "grep_search",
  render: ({ args, result, status }) => {
    const [expanded, setExpanded] = useState(false);

    const groups = result ? parseGrepOutput(result) : [];
    const totalMatches = groups.reduce(
      (sum, g) => sum + g.matches.filter((m) => m.isMatch).length,
      0,
    );

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
            <SearchIcon className="size-4 text-muted-foreground" />
          )}
          <span className="font-mono text-xs text-foreground">
            {args?.pattern ?? "search"}
          </span>
          {status.type === "complete" && (
            <span className="text-xs text-muted-foreground">
              {totalMatches} match{totalMatches !== 1 ? "es" : ""} in{" "}
              {groups.length} file{groups.length !== 1 ? "s" : ""}
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
            Searching for <code>{args?.pattern}</code>
            {args?.path ? ` in ${args.path}` : ""}…
          </div>
        )}

        {/* Results body */}
        {expanded && groups.length > 0 && (
          <div className="border-t border-border/50">
            {groups.map((group, gi) => (
              <div key={gi}>
                <div className="flex items-center gap-1.5 bg-muted/50 px-3 py-1 text-xs">
                  <FileTextIcon className="size-3 text-muted-foreground" />
                  <span className="font-medium text-foreground">
                    {group.file}
                  </span>
                </div>
                <pre className="overflow-x-auto bg-[#1e1e2e] px-3 py-1 text-xs leading-relaxed">
                  {group.matches.map((m, mi) => (
                    <div
                      key={mi}
                      className={cn(
                        "flex",
                        m.isMatch ? "bg-yellow-500/10" : "",
                      )}
                    >
                      <span className="mr-3 inline-block w-8 text-right text-muted-foreground">
                        {m.lineNumber}
                      </span>
                      <span
                        className={
                          m.isMatch ? "text-foreground" : "text-muted-foreground"
                        }
                      >
                        {m.content}
                      </span>
                    </div>
                  ))}
                </pre>
              </div>
            ))}
          </div>
        )}

        {/* No results */}
        {expanded && status.type === "complete" && groups.length === 0 && (
          <div className="border-t border-border/50 px-3 py-2 text-xs text-muted-foreground">
            No matches found.
          </div>
        )}
      </div>
    );
  },
});
