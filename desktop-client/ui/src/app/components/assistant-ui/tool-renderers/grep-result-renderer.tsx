/**
 * GrepResultRenderer — inline trigger + collapsible search results
 *
 * 文件名头部可点击，跳转到系统默认编辑器对应行号。
 */
import { useCallback } from "react";
import { makeAssistantToolUI } from "@assistant-ui/react";
import { FileTextIcon } from "lucide-react";
import { cn } from "@/app/components/ui/utils";
import { fileOpsApi } from "@utils/tauri";
import { CollapsibleToolShell } from "./tool-ui-shared";

interface GrepSearchArgs {
  pattern: string;
  path?: string;
  context_before?: number;
  context_after?: number;
  max_results?: number;
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

function parseGrepOutput(text: string): GrepFileGroup[] {
  const groups: GrepFileGroup[] = [];
  let current: GrepFileGroup | null = null;

  for (const line of text.split("\n")) {
    const fileMatch = line.match(/^──\s+(.+?)\s+──$/);
    if (fileMatch) {
      current = { file: fileMatch[1], matches: [] };
      groups.push(current);
      continue;
    }
    const lineMatch = line.match(/^\s*(\d+):\s(.*)$/);
    if (lineMatch && current) {
      current.matches.push({
        lineNumber: parseInt(lineMatch[1], 10),
        content: lineMatch[2],
        isMatch: true,
      });
      continue;
    }
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

function ClickableFileHeader({
  file,
  firstMatchLine,
}: {
  file: string;
  firstMatchLine?: number;
}) {
  const handleClick = useCallback(() => {
    fileOpsApi.openFileAtLine(file, firstMatchLine ?? 1).catch(() => {});
  }, [file, firstMatchLine]);

  return (
    <button
      type="button"
      onClick={handleClick}
      className="flex w-full items-center gap-1.5 bg-muted/80 px-3 py-1 text-xs text-left transition-colors hover:bg-muted"
    >
      <FileTextIcon className="size-3 shrink-0" />
      <span className="font-medium text-foreground underline decoration-dotted underline-offset-2">
        {file}
      </span>
    </button>
  );
}

export const GrepResultToolUI = makeAssistantToolUI<GrepSearchArgs, string>({
  toolName: "grep_search",
  render: ({ args, result, status }) => {
    const groups = result ? parseGrepOutput(result) : [];
    const totalMatches = groups.reduce(
      (sum, g) => sum + g.matches.filter((m) => m.isMatch).length,
      0,
    );

    return (
      <CollapsibleToolShell
        toolName="grep_search"
        status={status}
        summary={
          <>
            {args?.pattern && (
              <span className="truncate font-mono opacity-60">
                {args.pattern}
              </span>
            )}
            {status.type === "complete" && (
              <span className="shrink-0 opacity-40">
                {totalMatches} match{totalMatches !== 1 ? "es" : ""} in{" "}
                {groups.length} file{groups.length !== 1 ? "s" : ""}
              </span>
            )}
          </>
        }
      >
        {groups.length > 0 ? (
          <div className="max-h-96 overflow-y-auto rounded-md border bg-muted/50">
            {groups.map((group, gi) => (
              <div key={gi}>
                <ClickableFileHeader
                  file={group.file}
                  firstMatchLine={
                    group.matches.find((m) => m.isMatch)?.lineNumber
                  }
                />
                <pre className="overflow-x-auto px-3 py-1 text-xs leading-relaxed">
                  {group.matches.map((m, mi) => (
                    <div
                      key={mi}
                      className={cn("flex", m.isMatch && "bg-yellow-500/10")}
                    >
                      <span className="mr-3 inline-block w-8 text-right text-muted-foreground">
                        {m.lineNumber}
                      </span>
                      <span
                        className={
                          m.isMatch
                            ? "text-foreground"
                            : "text-muted-foreground"
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
        ) : status.type === "complete" ? (
          <div className="rounded-md border bg-muted/50 p-3 text-xs text-muted-foreground">
            No matches found.
          </div>
        ) : null}
      </CollapsibleToolShell>
    );
  },
});
