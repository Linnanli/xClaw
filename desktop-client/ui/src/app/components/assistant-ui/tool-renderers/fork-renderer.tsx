/**
 * ForkRenderer — session_fork 工具的 inline 渲染器。
 */
import { makeAssistantToolUI } from "@assistant-ui/react";
import { ToolCallShell } from "./tool-ui-shared";

interface SessionForkArgs {
  at_turn?: number;
  reason?: string;
}

export const SessionForkToolUI = makeAssistantToolUI<SessionForkArgs, string>({
  toolName: "session_fork",
  render: ({ args, status }) => (
    <ToolCallShell toolName="session_fork" status={status}>
      {args?.at_turn != null && (
        <span className="opacity-60">from turn {args.at_turn}</span>
      )}
      {args?.reason && (
        <span className="opacity-40">{args.reason}</span>
      )}
    </ToolCallShell>
  ),
});
