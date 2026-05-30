/**
 * SubAgentRenderer — inline trigger + collapsible details
 */
import { makeAssistantToolUI } from "@assistant-ui/react";
import { CollapsibleToolShell } from "./tool-ui-shared";

interface SubAgentArgs {
  role?: string;
  goal?: string;
  max_turns?: number;
}

interface SubAgentResult {
  action: string;
  role: string;
  goal: string;
  tool_whitelist: string[];
  max_turns: number;
  depth: number;
}

function parseSubAgentResult(raw: string): SubAgentResult | null {
  try {
    return JSON.parse(raw) as SubAgentResult;
  } catch {
    return null;
  }
}

export const SubAgentToolUI = makeAssistantToolUI<SubAgentArgs, string>({
  toolName: "sub_agent",
  render: ({ args, result, status }) => {
    const parsed =
      status.type === "complete" && result
        ? parseSubAgentResult(result)
        : null;

    return (
      <CollapsibleToolShell
        toolName="sub_agent"
        status={status}
        summary={
          <>
            {args?.role && (
              <span className="opacity-60">{args.role}</span>
            )}
            {args?.goal && (
              <span className="truncate opacity-40">{args.goal}</span>
            )}
          </>
        }
      >
        {parsed && (
          <div className="space-y-2 rounded-md border bg-muted/50 p-3 text-xs">
            <div className="text-foreground">{parsed.goal}</div>
            <div className="flex flex-wrap gap-1">
              {parsed.tool_whitelist.map((tool) => (
                <code
                  key={tool}
                  className="rounded bg-muted px-1.5 py-0.5 text-[10px] text-muted-foreground"
                >
                  {tool}
                </code>
              ))}
            </div>
            <div className="text-muted-foreground">
              Depth: {parsed.depth} · Max turns: {parsed.max_turns}
            </div>
          </div>
        )}
      </CollapsibleToolShell>
    );
  },
});
