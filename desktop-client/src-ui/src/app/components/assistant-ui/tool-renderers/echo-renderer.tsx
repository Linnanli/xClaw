/**
 * Echo tool — 纯 inline 单行渲染。
 */
import { makeAssistantToolUI } from "@assistant-ui/react";
import { ToolCallShell } from "./tool-ui-shared";

interface EchoArgs {
  message: string;
}

export const EchoToolUI = makeAssistantToolUI<EchoArgs, string>({
  toolName: "echo",
  render: ({ args, result, status }) => (
    <ToolCallShell toolName="echo" status={status}>
      {args?.message && <span className="opacity-60">{args.message}</span>}
      {status.type === "complete" && result && (
        <span className="opacity-40">→ {String(result)}</span>
      )}
    </ToolCallShell>
  ),
});
