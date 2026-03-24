/**
 * ChatWelcome - 欢迎页面
 *
 * Pencil 设计稿规范：
 * - 副标题：fontSize:15, fontWeight:500, fill:#6D6C6A
 * - 主标题：fontSize:26, fontWeight:700, letterSpacing:-0.5, fill:#1A1918
 * - 容器：gap:20, layout:vertical, alignItems:center
 * - 输入框宽度：720px
 */

import { QuickActions, type QuickAction } from './QuickActions';
import { ChatInput, type ChatInputProps } from './ChatInput';

export interface ChatWelcomeProps extends ChatInputProps {
  quickActions: QuickAction[];
  onQuickAction: (prompt: string) => void;
}

export function ChatWelcome({
  quickActions,
  onQuickAction,
  ...inputProps
}: ChatWelcomeProps) {
  return (
    <div className="flex flex-1 flex-col items-center justify-center px-10 py-10">
      <div className="flex flex-col items-center gap-5">
        <p className="text-[15px] font-medium text-text-secondary">
          你的专属 AI 团队已就绪
        </p>
        <h2 className="text-[26px] font-bold tracking-[-0.5px] text-foreground">
          今天需要我帮你做些什么？
        </h2>

        <QuickActions actions={quickActions} onSelect={onQuickAction} />

        <div className="w-[720px] max-w-full">
          <ChatInput {...inputProps} />
        </div>
      </div>
    </div>
  );
}
