/**
 * GitDiffRenderer — inline trigger + collapsible diff
 */
import { makeAssistantToolUI } from "@assistant-ui/react";
import { CollapsibleToolShell } from "./tool-ui-shared";

interface GitDiffArgs {
  staged?: boolean;
  file_path?: string;
  path?: string;
}

function DiffView({ text }: { text: string }) {
  return (
    <pre className="overflow-x-auto text-xs leading-relaxed">
      {text.split("\n").map((line, i) => {
        let cls = "";
        if (line.startsWith("+") && !line.startsWith("+++"))
          cls = "bg-green-500/15 text-green-400";
        else if (line.startsWith("-") && !line.startsWith("---"))
          cls = "bg-red-500/15 text-red-400";
        else if (line.startsWith("@@")) cls = "text-blue-400";
        else if (line.startsWith("diff --git"))
          cls = "text-yellow-400 font-semibold";
        return (
          <div key={i} className={cls}>
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
    const label = args?.staged ? "staged" : "unstaged";
    const noChanges =
      status.type === "complete" &&
      result?.includes("No ") &&
      result?.includes("changes");

    return (
      <CollapsibleToolShell
        toolName="git_diff"
        status={status}
        summary={
          <>
            <span className="opacity-60">{label}</span>
            {args?.file_path && (
              <span className="truncate opacity-40">{args.file_path}</span>
            )}
            {noChanges && <span className="opacity-40">no changes</span>}
          </>
        }
      >
        {status.type === "complete" && result && (
          <div className="max-h-96 overflow-y-auto rounded-md border bg-muted/50 p-3">
            <DiffView text={result} />
          </div>
        )}
      </CollapsibleToolShell>
    );
  },
});
