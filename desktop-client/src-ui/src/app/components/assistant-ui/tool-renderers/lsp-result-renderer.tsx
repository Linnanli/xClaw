/**
 * LspResultRenderer — lsp_query 工具的渲染器。
 *
 * 根据 action 类型（diagnostics / hover / goto_definition / find_references /
 * document_symbols / rename / completions）分别渲染不同格式的 LSP 结果。
 */
import { makeAssistantToolUI } from "@assistant-ui/react";
import { useState } from "react";
import {
  CodeIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  LoaderIcon,
} from "lucide-react";

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
            <div key={i} className={`px-2 font-mono ${cls}`}>
              {line || "\u00A0"}
            </div>
          );
        })}
      </div>
    );
  }

  if (action === "hover") {
    return (
      <pre className="overflow-x-auto whitespace-pre-wrap px-2 text-xs leading-relaxed text-foreground">
        {text}
      </pre>
    );
  }

  return (
    <pre className="overflow-x-auto text-xs leading-relaxed">
      {lines.map((line, i) => {
        let className = "px-2 text-foreground";
        if (line.match(/^\s+\S+:\d+/)) {
          className = "px-2 text-blue-400";
        } else if (line.startsWith("Found") || line.startsWith("No ")) {
          className = "px-2 text-muted-foreground";
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

export const LspResultToolUI = makeAssistantToolUI<LspQueryArgs, string>({
  toolName: "lsp_query",
  render: ({ args, result, status }) => {
    const [expanded, setExpanded] = useState(false);
    const action = args?.action ?? "hover";
    const label = ACTION_LABELS[action] ?? action;
    const filePath = args?.file_path;
    const isRunning = status.type === "running";

    return (
      <div className="my-1 overflow-hidden rounded-md border border-border/50 bg-muted/30 text-sm">
        <button
          type="button"
          onClick={() => setExpanded((p) => !p)}
          className="flex w-full items-center gap-2 px-3 py-2 text-left hover:bg-muted/50"
        >
          {isRunning ? (
            <LoaderIcon className="h-4 w-4 animate-spin text-muted-foreground" />
          ) : expanded ? (
            <ChevronDownIcon className="h-4 w-4 text-muted-foreground" />
          ) : (
            <ChevronRightIcon className="h-4 w-4 text-muted-foreground" />
          )}
          <CodeIcon className="h-4 w-4 text-purple-400" />
          <span className="font-medium text-foreground">{label}</span>
          {filePath && (
            <span className="truncate text-muted-foreground">— {filePath}</span>
          )}
          {args?.line != null && (
            <span className="text-muted-foreground text-xs">
              L{args.line + 1}
            </span>
          )}
        </button>

        {expanded && status.type === "complete" && result && (
          <div className="border-t border-border/50 bg-[#1e1e1e] p-2">
            <LspResultBody text={result} action={action} />
          </div>
        )}
      </div>
    );
  },
});
