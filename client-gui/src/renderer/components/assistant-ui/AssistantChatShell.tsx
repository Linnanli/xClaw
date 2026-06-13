import type { ReactNode, RefObject } from 'react';
import { AssistantRuntimeAdapter } from './runtime/AssistantRuntimeAdapter';
import type { Message } from '../../types';

interface AssistantChatShellProps {
  title: string;
  headerLabel: string;
  messages: Message[];
  isRunning: boolean;
  isSendDisabled: boolean;
  onNew: () => Promise<void> | void;
  onCancel: () => Promise<void> | void;
  scrollContainerRef: RefObject<HTMLDivElement>;
  messagesContainerRef: RefObject<HTMLDivElement>;
  messagesEndRef: RefObject<HTMLDivElement>;
  thread: ReactNode;
  statusIndicators?: ReactNode;
  composer: ReactNode;
}

export function AssistantChatShell({
  title,
  headerLabel,
  messages,
  isRunning,
  isSendDisabled,
  onNew,
  onCancel,
  scrollContainerRef,
  messagesContainerRef,
  messagesEndRef,
  thread,
  statusIndicators,
  composer,
}: AssistantChatShellProps): JSX.Element {
  return (
    <section
      className="flex-1 flex flex-col overflow-hidden bg-background"
      data-assistant-chat-shell="client-gui"
    >
      <header
        className="relative h-12 border-b border-border-muted grid grid-cols-[1fr_auto_1fr] items-center px-4 lg:px-8 bg-background/88 backdrop-blur-md"
        data-assistant-chat-header="client-gui"
      >
        <div className="text-[11px] font-medium tracking-[0.08em] uppercase text-text-muted">
          {headerLabel}
        </div>
        <h2 className="text-[15px] font-medium text-text-primary text-center truncate max-w-[40vw] lg:max-w-[32rem]">
          {title}
        </h2>
        <div />
      </header>

      <AssistantRuntimeAdapter
        messages={messages}
        isRunning={isRunning}
        isSendDisabled={isSendDisabled}
        onNew={onNew}
        onCancel={onCancel}
      >
        <div
          ref={scrollContainerRef}
          className="flex-1 overflow-y-auto"
          data-assistant-scroll-container="client-gui"
        >
          <div
            ref={messagesContainerRef}
            className="w-full max-w-[920px] mx-auto py-8 px-5 lg:px-8 space-y-5"
            data-assistant-message-container="client-gui"
          >
            {thread}
            {statusIndicators}
            <div ref={messagesEndRef} />
          </div>
        </div>

        <div
          className="border-t border-border-muted bg-background/92 backdrop-blur-md"
          data-assistant-composer-bar="client-gui"
        >
          <div
            className="max-w-[920px] mx-auto px-5 lg:px-8 py-5"
            data-assistant-composer-container="client-gui"
          >
            {composer}
          </div>
        </div>
      </AssistantRuntimeAdapter>
    </section>
  );
}
