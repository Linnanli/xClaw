/**
 * PlanRenderer — plan_mode 工具的渲染器。
 *
 * 根据 action 类型（toggle / status / submit）分别渲染：
 * - toggle: 模式切换状态
 * - status: 当前 Plan Mode 状态
 * - submit: 结构化计划卡片（步骤、风险、文件 + 审批按钮）
 */
import { makeAssistantToolUI } from "@assistant-ui/react";
import { useState } from "react";
import {
  ClipboardListIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  LoaderIcon,
  CheckCircle2Icon,
  XCircleIcon,
  ShieldAlertIcon,
  FileIcon,
} from "lucide-react";

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

function riskBadge(risk: string) {
  if (risk === "high") {
    return (
      <span className="inline-flex items-center gap-1 rounded-full bg-red-500/15 px-1.5 py-0.5 text-[10px] font-medium text-red-400">
        <ShieldAlertIcon className="h-3 w-3" />
        High
      </span>
    );
  }
  if (risk === "medium") {
    return (
      <span className="rounded-full bg-yellow-500/15 px-1.5 py-0.5 text-[10px] font-medium text-yellow-400">
        Medium
      </span>
    );
  }
  return (
    <span className="rounded-full bg-green-500/15 px-1.5 py-0.5 text-[10px] font-medium text-green-400">
      Low
    </span>
  );
}

function StepCard({
  step,
}: {
  step: { step: number; description: string; tool_name: string; risk: string; files: string[] };
}) {
  return (
    <div className="flex items-start gap-2 rounded border border-border/30 bg-background/50 px-3 py-2">
      <span className="mt-0.5 flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-muted text-[10px] font-bold text-muted-foreground">
        {step.step}
      </span>
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="text-xs text-foreground">{step.description}</span>
          {riskBadge(step.risk)}
        </div>
        <div className="mt-0.5 flex items-center gap-2 text-[10px] text-muted-foreground">
          <code className="rounded bg-muted px-1">{step.tool_name}</code>
          {step.files.length > 0 && (
            <span className="flex items-center gap-0.5">
              <FileIcon className="h-3 w-3" />
              {step.files.join(", ")}
            </span>
          )}
        </div>
      </div>
    </div>
  );
}

export const PlanModeToolUI = makeAssistantToolUI<PlanModeArgs, string>({
  toolName: "plan_mode",
  render: ({ args, result, status }) => {
    const [expanded, setExpanded] = useState(true);
    const isRunning = status.type === "running";
    const action = args?.action ?? "status";

    let parsed: PlanResult | null = null;
    if (status.type === "complete" && result) {
      try {
        parsed = JSON.parse(String(result));
      } catch {
        parsed = null;
      }
    }

    // Toggle action — simple status card
    if (action === "toggle" || action === "status") {
      return (
        <div className="my-1 rounded-md border border-border/50 bg-muted/30 px-3 py-2 text-sm">
          <div className="flex items-center gap-2">
            <ClipboardListIcon className="h-4 w-4 text-blue-400" />
            <span className="font-medium text-foreground">
              {action === "toggle" ? "Plan Mode Toggle" : "Plan Mode Status"}
            </span>
            {isRunning && (
              <LoaderIcon className="h-3 w-3 animate-spin text-muted-foreground" />
            )}
            {parsed?.plan_mode !== undefined && (
              <span
                className={`rounded-full px-2 py-0.5 text-[10px] font-medium ${
                  parsed.plan_mode
                    ? "bg-blue-500/15 text-blue-400"
                    : "bg-muted text-muted-foreground"
                }`}
              >
                {parsed.plan_mode ? "Planning" : "Execution"}
              </span>
            )}
          </div>
        </div>
      );
    }

    // Submit action — full plan card with steps
    return (
      <div className="my-1 overflow-hidden rounded-md border border-blue-500/30 bg-blue-500/5 text-sm">
        <button
          type="button"
          onClick={() => setExpanded((p) => !p)}
          className="flex w-full items-center gap-2 px-3 py-2 text-left hover:bg-blue-500/10"
        >
          {isRunning ? (
            <LoaderIcon className="h-4 w-4 animate-spin text-blue-400" />
          ) : expanded ? (
            <ChevronDownIcon className="h-4 w-4 text-blue-400" />
          ) : (
            <ChevronRightIcon className="h-4 w-4 text-blue-400" />
          )}
          <ClipboardListIcon className="h-4 w-4 text-blue-400" />
          <span className="font-medium text-foreground">Execution Plan</span>
          {parsed?.goal && (
            <span className="truncate text-muted-foreground">
              — {parsed.goal}
            </span>
          )}
          {parsed?.steps_count != null && (
            <span className="ml-auto shrink-0 rounded-full bg-muted px-1.5 text-[10px] text-muted-foreground">
              {parsed.steps_count} steps
            </span>
          )}
        </button>

        {expanded && parsed && (
          <div className="border-t border-blue-500/20 px-3 py-2">
            {/* Confidence */}
            {parsed.confidence != null && (
              <div className="mb-2 flex items-center gap-2 text-xs text-muted-foreground">
                <span>Confidence:</span>
                <div className="h-1.5 w-20 overflow-hidden rounded-full bg-muted">
                  <div
                    className="h-full rounded-full bg-blue-400"
                    style={{ width: `${Math.round(parsed.confidence * 100)}%` }}
                  />
                </div>
                <span>{Math.round(parsed.confidence * 100)}%</span>
              </div>
            )}

            {/* Steps */}
            {parsed.steps && (
              <div className="space-y-1.5">
                {parsed.steps.map((step) => (
                  <StepCard key={step.step} step={step} />
                ))}
              </div>
            )}

            {/* Summary */}
            {parsed.steps && (
              <div className="mt-2 flex items-center gap-3 text-[10px] text-muted-foreground">
                <span className="flex items-center gap-1">
                  <CheckCircle2Icon className="h-3 w-3 text-green-400" />
                  {parsed.steps.filter((s) => s.risk === "low").length} low risk
                </span>
                <span className="flex items-center gap-1">
                  <ShieldAlertIcon className="h-3 w-3 text-yellow-400" />
                  {parsed.steps.filter((s) => s.risk === "medium").length} medium
                </span>
                <span className="flex items-center gap-1">
                  <XCircleIcon className="h-3 w-3 text-red-400" />
                  {parsed.steps.filter((s) => s.risk === "high").length} high
                </span>
              </div>
            )}
          </div>
        )}
      </div>
    );
  },
});
