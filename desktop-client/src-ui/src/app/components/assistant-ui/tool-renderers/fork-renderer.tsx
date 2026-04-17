/**
 * ForkRenderer — session_fork 工具的渲染器。
 *
 * 显示 Fork 意图（从哪个 Turn 分支）及执行状态。
 */
import { makeAssistantToolUI } from "@assistant-ui/react";
import {
  GitBranchIcon,
  LoaderIcon,
} from "lucide-react";

interface SessionForkArgs {
  at_turn?: number;
  reason?: string;
}

interface ForkResult {
  action: string;
  at_turn: number;
  reason?: string;
  new_thread_id?: string;
}

export const SessionForkToolUI = makeAssistantToolUI<SessionForkArgs, string>({
  toolName: "session_fork",
  render: ({ args, result, status }) => {
    const isRunning = status.type === "running";
    const atTurn = args?.at_turn;

    let parsed: ForkResult | null = null;
    if (status.type === "complete" && result) {
      try {
        parsed = JSON.parse(String(result));
      } catch {
        parsed = null;
      }
    }

    return (
      <div className="my-1 rounded-md border border-purple-500/30 bg-purple-500/5 px-3 py-2 text-sm">
        <div className="flex items-center gap-2">
          <GitBranchIcon className="h-4 w-4 text-purple-400" />
          <span className="font-medium text-foreground">Session Fork</span>
          {isRunning && (
            <LoaderIcon className="h-3 w-3 animate-spin text-muted-foreground" />
          )}
          {atTurn != null && (
            <span className="rounded-full bg-purple-500/15 px-2 py-0.5 text-[10px] font-medium text-purple-400">
              from turn {atTurn}
            </span>
          )}
        </div>
        {args?.reason && (
          <p className="mt-1 text-xs text-muted-foreground">{args.reason}</p>
        )}
        {parsed?.new_thread_id && (
          <p className="mt-1 text-xs text-muted-foreground">
            New branch: <code className="rounded bg-muted px-1">{parsed.new_thread_id}</code>
          </p>
        )}
      </div>
    );
  },
});
