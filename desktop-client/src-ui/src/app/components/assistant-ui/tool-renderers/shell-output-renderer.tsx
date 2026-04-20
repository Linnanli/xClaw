/**
 * ShellOutputRenderer — inline trigger + collapsible terminal output
 *
 * 默认折叠后只展示前 PREVIEW_LINES 行，点击 "展开全部" 可查看完整输出。
 */
import { useState } from "react";
import { makeAssistantToolUI } from "@assistant-ui/react";
import { CollapsibleToolShell, truncate } from "./tool-ui-shared";

const PREVIEW_LINES = 5;

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

function parseShellResult(raw: string): ShellResult | null {
  try {
    return JSON.parse(raw) as ShellResult;
  } catch {
    return null;
  }
}

function ShellOutput({ text }: { text: string }) {
  const lines = text.split("\n");
  const needsTruncation = lines.length > PREVIEW_LINES;
  const [expanded, setExpanded] = useState(false);
  const visible = expanded ? text : lines.slice(0, PREVIEW_LINES).join("\n");

  return (
    <div className="rounded-md border bg-muted/50">
      <pre className="max-h-96 overflow-y-auto whitespace-pre-wrap break-all p-3 font-mono text-xs">
        {visible}
      </pre>
      {needsTruncation && (
        <button
          onClick={() => setExpanded((v) => !v)}
          className="w-full border-t px-3 py-1 text-xs text-muted-foreground transition-colors hover:bg-muted"
        >
          {expanded
            ? "收起"
            : `展开全部（共 ${lines.length} 行）`}
        </button>
      )}
    </div>
  );
}

export const ShellOutputToolUI = makeAssistantToolUI<ShellArgs, string>({
  toolName: "shell",
  render: ({ args, result, status }) => {
    const parsed =
      status.type === "complete" && result ? parseShellResult(result) : null;
    const exitCode = parsed?.exit_code ?? null;
    const output =
      parsed?.output ?? (status.type === "complete" ? result : null);

    return (
      <CollapsibleToolShell
        toolName="shell"
        status={status}
        summary={
          <>
            {args?.command && (
              <span className="truncate font-mono opacity-60">
                {truncate(args.command, 60)}
              </span>
            )}
            {exitCode != null && exitCode !== 0 && (
              <span className="shrink-0 text-destructive">
                exit {exitCode}
              </span>
            )}
          </>
        }
      >
        {output && <ShellOutput text={output} />}
      </CollapsibleToolShell>
    );
  },
});
