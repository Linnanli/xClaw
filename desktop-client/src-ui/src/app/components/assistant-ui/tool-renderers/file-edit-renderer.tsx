/**
 * FileEditRenderer — inline trigger + collapsible diff + Undo
 *
 * 默认 auto-apply：Agent 的 code_edit 工具直接写入文件，
 * 前端展示 diff + Undo 按钮。用户不满意可撤回。
 */
import { useCallback, useState } from "react";
import { makeAssistantToolUI } from "@assistant-ui/react";
import { Undo2Icon } from "lucide-react";
import { fileOpsApi } from "@utils/tauri";
import { CollapsibleToolShell, basename } from "./tool-ui-shared";

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
  return (
    <pre className="overflow-x-auto text-xs leading-relaxed">
      {diff.split("\n").map((line, i) => {
        let cls = "";
        if (line.startsWith("+") && !line.startsWith("+++"))
          cls = "bg-green-500/15 text-green-400";
        else if (line.startsWith("-") && !line.startsWith("---"))
          cls = "bg-red-500/15 text-red-400";
        else if (line.startsWith("@@")) cls = "text-blue-400";
        return (
          <div key={i} className={cls}>
            {line || "\u00A0"}
          </div>
        );
      })}
    </pre>
  );
}

function parseEditResult(raw: string): CodeEditResult | null {
  try {
    return JSON.parse(raw) as CodeEditResult;
  } catch {
    return null;
  }
}

type UndoState = "idle" | "pending" | "done" | "error";

function UndoButton({
  args,
  parsed,
}: {
  args: CodeEditArgs | undefined;
  parsed: CodeEditResult;
}) {
  const [undoState, setUndoState] = useState<UndoState>("idle");
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  const handleUndo = useCallback(async () => {
    if (!args) return;
    setUndoState("pending");
    setErrorMsg(null);
    try {
      await fileOpsApi.undoFileEdit(
        parsed.path,
        args.old_string,
        args.new_string,
        parsed.replaced_count,
      );
      setUndoState("done");
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
      setUndoState("error");
    }
  }, [args, parsed]);

  if (undoState === "done") {
    return (
      <span className="text-xs text-muted-foreground">已撤回</span>
    );
  }

  return (
    <>
      <button
        onClick={handleUndo}
        disabled={undoState === "pending"}
        className="flex items-center gap-1 rounded-md border border-border px-2 py-1 text-xs text-muted-foreground transition-colors hover:bg-muted disabled:opacity-50"
      >
        <Undo2Icon className="size-3" />
        {undoState === "pending" ? "撤回中..." : "撤回编辑"}
      </button>
      {errorMsg && (
        <span className="text-xs text-destructive">{errorMsg}</span>
      )}
    </>
  );
}

export const FileEditToolUI = makeAssistantToolUI<CodeEditArgs, string>({
  toolName: "code_edit",
  render: ({ args, result, status }) => {
    const parsed =
      status.type === "complete" && result ? parseEditResult(result) : null;
    const filePath = parsed?.path ?? args?.file_path ?? "unknown";
    const fileName = basename(filePath);
    const isComplete = status.type === "complete";

    const undoAction =
      isComplete && parsed?.success && args ? (
        <UndoButton args={args} parsed={parsed} />
      ) : null;

    return (
      <CollapsibleToolShell
        toolName="code_edit"
        status={status}
        summary={
          <>
            <span className="truncate opacity-60">{fileName}</span>
            {parsed && (
              <span className="shrink-0 opacity-40">
                {parsed.replaced_count} replacement
                {parsed.replaced_count !== 1 ? "s" : ""}
              </span>
            )}
          </>
        }
        actions={undoAction}
      >
        {parsed?.diff_preview ? (
          <div className="max-h-96 overflow-y-auto rounded-md border bg-muted/50 p-3">
            <DiffView diff={parsed.diff_preview} />
          </div>
        ) : isComplete && result && !parsed ? (
          <div className="rounded-md border bg-muted/50 p-3 text-xs text-destructive">
            {result}
          </div>
        ) : null}
      </CollapsibleToolShell>
    );
  },
});
