/**
 * PlanRenderer — toggle/status 纯 inline，submit 带折叠
 */
import { makeAssistantToolUI } from "@assistant-ui/react";
import { FileIcon } from "lucide-react";
import { ToolCallShell, CollapsibleToolShell } from "./tool-ui-shared";

interface PlanModeArgs {
  action?: string;
  plan?: {
    goal?: string;
    steps?: PlanStep[];
    confidence?: number;
  };
}

interface PlanStep {
  description: string;
  tool_name?: string;
  risk?: string;
  files?: string[];
}

interface PlanResult {
  action: string;
  plan_mode?: boolean;
  plan_id?: string;
  goal?: string;
  steps_count?: number;
  steps?: Array<{
    step: number;
    description: string;
    tool_name: string;
    risk: string;
    files: string[];
  }>;
  confidence?: number;
}

function riskColor(risk: string): string {
  if (risk === "high") return "text-red-400";
  if (risk === "medium") return "text-yellow-400";
  return "text-green-400";
}

function parsePlanResult(raw: string): PlanResult | null {
  try {
    return JSON.parse(raw) as PlanResult;
  } catch {
    return null;
  }
}

export const PlanModeToolUI = makeAssistantToolUI<PlanModeArgs, string>({
  toolName: "plan_mode",
  render: ({ args, result, status }) => {
    const action = args?.action ?? "status";
    const parsed =
      status.type === "complete" && result ? parsePlanResult(result) : null;

    if (action === "toggle" || action === "status") {
      return (
        <ToolCallShell toolName="plan_mode" status={status}>
          <span className="opacity-60">{action}</span>
          {parsed?.plan_mode !== undefined && (
            <span className="opacity-40">
              {parsed.plan_mode ? "planning" : "execution"}
            </span>
          )}
        </ToolCallShell>
      );
    }

    return (
      <CollapsibleToolShell
        toolName="plan_mode"
        status={status}
        summary={
          <>
            <span className="opacity-60">submit</span>
            {parsed?.goal && (
              <span className="truncate opacity-40">{parsed.goal}</span>
            )}
            {parsed?.steps_count != null && (
              <span className="shrink-0 opacity-40">
                {parsed.steps_count} steps
              </span>
            )}
          </>
        }
      >
        {parsed?.steps && (
          <div className="max-h-96 space-y-2 overflow-y-auto rounded-md border bg-muted/50 p-3 text-xs">
            {parsed.confidence != null && (
              <div className="flex items-center gap-2 text-muted-foreground">
                <span>Confidence:</span>
                <div className="h-1.5 w-20 overflow-hidden rounded-full bg-muted">
                  <div
                    className="h-full rounded-full bg-blue-400"
                    style={{
                      width: `${Math.round(parsed.confidence * 100)}%`,
                    }}
                  />
                </div>
                <span>{Math.round(parsed.confidence * 100)}%</span>
              </div>
            )}
            {parsed.steps.map((step) => (
              <div key={step.step} className="flex items-start gap-2">
                <span className="mt-0.5 flex h-4 w-4 shrink-0 items-center justify-center rounded-full bg-muted text-[10px] font-bold">
                  {step.step}
                </span>
                <div className="min-w-0 flex-1">
                  <span className="text-foreground">{step.description}</span>
                  <div className="mt-0.5 flex items-center gap-2 text-[10px] text-muted-foreground">
                    <code className="rounded bg-muted px-1">
                      {step.tool_name}
                    </code>
                    <span className={riskColor(step.risk)}>{step.risk}</span>
                    {step.files.length > 0 && (
                      <span className="flex items-center gap-0.5">
                        <FileIcon className="size-3" />
                        {step.files.join(", ")}
                      </span>
                    )}
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </CollapsibleToolShell>
    );
  },
});
