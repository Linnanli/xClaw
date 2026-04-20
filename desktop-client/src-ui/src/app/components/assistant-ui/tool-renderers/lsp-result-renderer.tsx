/**
 * LspResultRenderer — inline trigger + collapsible LSP results
 */
import { makeAssistantToolUI } from "@assistant-ui/react";
import { CollapsibleToolShell, shortenPath } from "./tool-ui-shared";

interface LspQueryArgs {
  action?: string;
  file_path?: string;
  line?: number;
  column?: number;
  new_name?: string;
}

const ACTION_LABELS: Record<string, string> = {
  goto_definition: "Go to Definition",
  find_references: "Find References",
  diagnostics: "Diagnostics",
  hover: "Hover",
  document_symbols: "Document Symbols",
  rename: "Rename",
  completions: "Completions",
};

function severityClass(tag: string): string {
  if (tag === "ERROR") return "text-red-400";
  if (tag === "WARN") return "text-yellow-400";
  if (tag === "INFO") return "text-blue-400";
  return "text-muted-foreground";
}

function LspResultBody({ text, action }: { text: string; action: string }) {
  const lines = text.split("\n");

  if (action === "diagnostics") {
    return (
      <div className="space-y-0.5 text-xs">
        {lines.map((line, i) => {
          const match = line.match(/\[(ERROR|WARN|INFO|HINT|DIAG)\]/);
          const cls = match ? severityClass(match[1]) : "text-foreground";
          return (
            <div key={i} className={`font-mono ${cls}`}>
              {line || "\u00A0"}
            </div>
          );
        })}
      </div>
    );
  }

  if (action === "hover") {
    return (
      <pre className="overflow-x-auto whitespace-pre-wrap text-xs leading-relaxed text-foreground">
        {text}
      </pre>
    );
  }

  return (
    <pre className="overflow-x-auto text-xs leading-relaxed">
      {lines.map((line, i) => {
        let cls = "text-foreground";
        if (line.match(/^\s+\S+:\d+/)) cls = "text-blue-400";
        else if (line.startsWith("Found") || line.startsWith("No "))
          cls = "text-muted-foreground";
        return (
          <div key={i} className={cls}>
            {line || "\u00A0"}
          </div>
        );
      })}
    </pre>
  );
}

export const LspResultToolUI = makeAssistantToolUI<LspQueryArgs, string>({
  toolName: "lsp_query",
  render: ({ args, result, status }) => {
    const action = args?.action ?? "hover";
    const label = ACTION_LABELS[action] ?? action;

    return (
      <CollapsibleToolShell
        toolName="lsp_query"
        status={status}
        summary={
          <>
            <span className="opacity-60">{label}</span>
            {args?.file_path && (
              <span className="truncate opacity-40">
                {shortenPath(args.file_path)}
              </span>
            )}
            {args?.line != null && (
              <span className="opacity-40">L{args.line + 1}</span>
            )}
          </>
        }
      >
        {status.type === "complete" && result && (
          <div className="max-h-96 overflow-y-auto rounded-md border bg-muted/50 p-3">
            <LspResultBody text={result} action={action} />
          </div>
        )}
      </CollapsibleToolShell>
    );
  },
});
