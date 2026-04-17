/**
 * Echo tool custom renderer — P0-Pre-2 technical spike.
 *
 * Validates that `makeAssistantToolUI` from @assistant-ui/react correctly
 * populates `part.toolUI` in thread.tsx, replacing `<ToolFallback>`.
 *
 * Once confirmed working, this pattern will be replicated for:
 * - FileOperationGuard renderer (P0 Week 1)
 * - GrepSearch renderer (P0 Week 1)
 * - BashValidator renderer (P0 Week 2)
 */
import { makeAssistantToolUI } from "@assistant-ui/react";

interface EchoArgs {
  message: string;
}

export const EchoToolUI = makeAssistantToolUI<EchoArgs, string>({
  toolName: "echo",
  render: ({ args, result, status }) => (
    <div className="my-1 rounded-md border border-border/50 bg-muted/30 px-3 py-2 text-sm">
      <div className="flex items-center gap-2">
        <span className="font-medium text-muted-foreground">Echo</span>
        {status.type === "running" && (
          <span className="size-2 animate-pulse rounded-full bg-blue-400" />
        )}
      </div>
      {args?.message && (
        <p className="mt-1 text-foreground">{args.message}</p>
      )}
      {status.type === "complete" && result && (
        <p className="mt-1 text-muted-foreground">{String(result)}</p>
      )}
    </div>
  ),
});
