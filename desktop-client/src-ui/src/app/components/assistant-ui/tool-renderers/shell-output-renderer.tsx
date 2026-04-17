/**
 * ShellOutputRenderer — ShellTool 输出的终端风格渲染器。
 *
 * 显示：
 * - 命令 + exit code 指示器（绿/红圆点）
 * - 终端风格输出（黑底，长输出折叠前 20 行）
 * - stderr 红色区分
 * - 语义验证 warnings（如有）
 */
import { makeAssistantToolUI } from "@assistant-ui/react";
import { useState } from "react";
import {
  TerminalIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  AlertTriangleIcon,
} from "lucide-react";
import { cn } from "@/app/components/ui/utils";

interface ShellArgs {
  command: string;
  workdir?: string;
  timeout?: number;
}

interface ShellResult {
  output: string;
  exit_code: number;
  success: boolean;
  sandboxed: boolean;
  intent: string;
  warnings?: string[];
}

const COLLAPSED_LINE_LIMIT = 20;

export const ShellOutputToolUI = makeAssistantToolUI<ShellArgs, string>({
  toolName: "shell",
  render: ({ args, result, status }) => {
    const [expanded, setExpanded] = useState(false);
    const [showAll, setShowAll] = useState(false);

    let parsed: ShellResult | null = null;
    if (status.type === "complete" && result) {
      try {
        parsed = JSON.parse(result) as ShellResult;
      } catch {
        // plain text fallback
      }
    }

    const outputLines = parsed?.output?.split("\n") ?? [];
    const hasStderr = parsed?.output?.includes("--- stderr ---") ?? false;
    const needsTruncation =
      !showAll && outputLines.length > COLLAPSED_LINE_LIMIT;
    const visibleLines = needsTruncation
      ? outputLines.slice(0, COLLAPSED_LINE_LIMIT)
      : outputLines;

    const exitCode = parsed?.exit_code ?? null;

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
          ) : exitCode === 0 ? (
            <span className="size-2 rounded-full bg-green-500" />
          ) : exitCode !== null ? (
            <span className="size-2 rounded-full bg-red-500" />
          ) : (
            <TerminalIcon className="size-4 text-muted-foreground" />
          )}
          <code className="max-w-[70%] truncate text-xs text-foreground">
            {args?.command ?? "shell"}
          </code>
          {parsed?.sandboxed && (
            <span className="rounded bg-blue-500/20 px-1.5 py-0.5 text-[10px] text-blue-400">
              sandbox
            </span>
          )}
          {exitCode !== null && exitCode !== 0 && (
            <span className="text-xs text-red-400">exit {exitCode}</span>
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
            Running{args?.command ? `: ${args.command}` : ""}…
          </div>
        )}

        {/* Warnings */}
        {expanded && parsed?.warnings && parsed.warnings.length > 0 && (
          <div className="border-t border-border/50 bg-yellow-500/5 px-3 py-1.5">
            {parsed.warnings.map((w, i) => (
              <div
                key={i}
                className="flex items-start gap-1.5 text-xs text-yellow-500"
              >
                <AlertTriangleIcon className="mt-0.5 size-3 shrink-0" />
                <span>{w}</span>
              </div>
            ))}
          </div>
        )}

        {/* Terminal output */}
        {expanded && parsed?.output && (
          <div className="border-t border-border/50 bg-[#1e1e2e] px-3 py-2">
            <pre className="overflow-x-auto text-xs leading-relaxed">
              {visibleLines.map((line, i) => {
                const inStderr =
                  hasStderr &&
                  parsed!.output.indexOf("--- stderr ---") <=
                    parsed!.output.indexOf(line);
                return (
                  <div
                    key={i}
                    className={cn(inStderr ? "text-red-400" : "text-green-300")}
                  >
                    {line || "\u00A0"}
                  </div>
                );
              })}
            </pre>
            {needsTruncation && (
              <button
                type="button"
                onClick={() => setShowAll(true)}
                className="mt-1 text-xs text-blue-400 hover:underline"
              >
                Show all {outputLines.length} lines
              </button>
            )}
          </div>
        )}

        {/* Plain text fallback for non-JSON result */}
        {expanded && status.type === "complete" && result && !parsed && (
          <div className="border-t border-border/50 bg-[#1e1e2e] px-3 py-2">
            <pre className="overflow-x-auto text-xs leading-relaxed text-green-300">
              {result}
            </pre>
          </div>
        )}
      </div>
    );
  },
});
