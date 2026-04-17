/**
 * SubAgentRenderer — sub_agent 工具的渲染器。
 *
 * 显示子 Agent 的角色（Explore / Verify / Custom）、目标、
 * 工具白名单和执行状态。
 */
import { makeAssistantToolUI } from "@assistant-ui/react";
import { useState } from "react";
import {
  BotIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  LoaderIcon,
  SearchIcon,
  ShieldCheckIcon,
  WrenchIcon,
} from "lucide-react";

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

const ROLE_CONFIG: Record<string, { icon: typeof SearchIcon; color: string; label: string }> = {
  explore: { icon: SearchIcon, color: "text-cyan-400", label: "Explore" },
  verify: { icon: ShieldCheckIcon, color: "text-amber-400", label: "Verify" },
};

export const SubAgentToolUI = makeAssistantToolUI<SubAgentArgs, string>({
  toolName: "sub_agent",
  render: ({ args, result, status }) => {
    const [expanded, setExpanded] = useState(false);
    const isRunning = status.type === "running";
    const role = args?.role ?? "explore";
    const config = ROLE_CONFIG[role] ?? {
      icon: WrenchIcon,
      color: "text-muted-foreground",
      label: role,
    };

    let parsed: SubAgentResult | null = null;
    if (status.type === "complete" && result) {
      try {
        parsed = JSON.parse(String(result));
      } catch {
        parsed = null;
      }
    }

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
          <BotIcon className="h-4 w-4 text-muted-foreground" />
          <span className="font-medium text-foreground">Sub-Agent</span>
          <span
            className={`rounded-full px-2 py-0.5 text-[10px] font-medium ${config.color} bg-current/10`}
          >
            {config.label}
          </span>
          {args?.goal && (
            <span className="truncate text-muted-foreground">— {args.goal}</span>
          )}
          {parsed?.max_turns != null && (
            <span className="ml-auto shrink-0 text-[10px] text-muted-foreground">
              max {parsed.max_turns} turns
            </span>
          )}
        </button>

        {expanded && parsed && (
          <div className="border-t border-border/30 px-3 py-2">
            <div className="mb-1.5 text-xs text-foreground">{parsed.goal}</div>
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
            <div className="mt-1.5 text-[10px] text-muted-foreground">
              Depth: {parsed.depth} · Max turns: {parsed.max_turns}
            </div>
          </div>
        )}
      </div>
    );
  },
});
