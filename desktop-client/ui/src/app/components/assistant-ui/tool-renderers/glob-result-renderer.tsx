/**
 * GlobResultRenderer — inline trigger + collapsible file list
 *
 * 文件路径可点击，用系统默认编辑器打开。
 */
import { useCallback } from "react";
import { makeAssistantToolUI } from "@assistant-ui/react";
import { FileIcon } from "lucide-react";
import { fileOpsApi } from "@utils/tauri";
import { CollapsibleToolShell } from "./tool-ui-shared";

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
  const headerMatch = lines[0]?.match(/^Found (\d+) files:/);
  const count = headerMatch ? parseInt(headerMatch[1], 10) : 0;
  const files = lines
    .slice(1)
    .map((l) => l.trim())
    .filter((l) => l.length > 0);
  return { count, files };
}

function ClickableFilePath({ file }: { file: string }) {
  const handleClick = useCallback(() => {
    fileOpsApi.openFileAtLine(file, 1).catch(() => {});
  }, [file]);

  return (
    <button
      type="button"
      onClick={handleClick}
      className="flex items-center gap-1.5 py-0.5 text-left transition-colors hover:text-foreground"
    >
      <FileIcon className="size-3 shrink-0 text-muted-foreground" />
      <span className="font-mono text-foreground underline decoration-dotted underline-offset-2">
        {file}
      </span>
    </button>
  );
}

export const GlobResultToolUI = makeAssistantToolUI<GlobSearchArgs, string>({
  toolName: "glob_search",
  render: ({ args, result, status }) => {
    const { count, files } = result
      ? parseGlobOutput(result)
      : { count: 0, files: [] };

    return (
      <CollapsibleToolShell
        toolName="glob_search"
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
                {count} file{count !== 1 ? "s" : ""}
              </span>
            )}
          </>
        }
      >
        {files.length > 0 ? (
          <div className="max-h-96 overflow-y-auto rounded-md border bg-muted/50 p-3 text-xs">
            {files.map((file, i) => (
              <ClickableFilePath key={i} file={file} />
            ))}
          </div>
        ) : status.type === "complete" && count === 0 ? (
          <div className="rounded-md border bg-muted/50 p-3 text-xs text-muted-foreground">
            No files matched.
          </div>
        ) : null}
      </CollapsibleToolShell>
    );
  },
});
