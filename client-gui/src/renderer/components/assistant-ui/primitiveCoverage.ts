export interface AssistantPrimitiveCoverageRow {
  primitive: string;
  requiresProvider: boolean;
  canonicalStateRead: string;
  actionForwardingTarget: string;
  chosenImplementation: string;
  fallback: string;
}

export const assistantPrimitiveCoverage: AssistantPrimitiveCoverageRow[] = [
  {
    primitive: 'AssistantRuntimeProvider + useExternalStoreRuntime',
    requiresProvider: true,
    canonicalStateRead:
      'displayedMessages, running state, submitting state from client-gui store/hooks',
    actionForwardingTarget: 'onNew -> continueSession, onCancel -> stopSession',
    chosenImplementation: 'provider-backed derived runtime adapter',
    fallback:
      'keep client-gui store as source of truth; do not adopt react-ai-sdk runtime ownership',
  },
  {
    primitive: 'ThreadPrimitive.Root + ThreadPrimitive.Viewport + ThreadPrimitive.Messages',
    requiresProvider: true,
    canonicalStateRead: 'displayedMessages including partialMessage and partialThinking',
    actionForwardingTarget: 'none; render-only thread viewport',
    chosenImplementation: 'adapter-backed thread shell in AssistantThreadView',
    fallback: 'legacy message list renderer while preserving runtime adapter boundary',
  },
  {
    primitive: 'MessagePrimitive.Root',
    requiresProvider: true,
    canonicalStateRead: 'Message metadata and ContentBlock[] from client-gui store',
    actionForwardingTarget:
      'message-level actions stay local unless the primitive owns the action safely',
    chosenImplementation: 'assistant-ui message root with existing ContentBlockView rendering',
    fallback: 'legacy AssistantMessage internals inside the primitive root',
  },
  {
    primitive: 'ActionBarPrimitive.Root + ActionBarPrimitive.Copy',
    requiresProvider: true,
    canonicalStateRead: 'assistant-ui converted message text and copied state',
    actionForwardingTarget: 'navigator.clipboard through assistant-ui action primitive',
    chosenImplementation: 'assistant-ui copy action primitive with existing icon/button styling',
    fallback: 'legacy local copy handler if the primitive cannot provide clipboard behavior',
  },
  {
    primitive: 'ComposerPrimitive.Root',
    requiresProvider: true,
    canonicalStateRead: 'prompt and draft attachments remain controlled by ChatView/WelcomeView',
    actionForwardingTarget:
      'submit -> existing onSubmit; runtime onNew is intentionally bypassed by preventDefault while composer remains controlled',
    chosenImplementation:
      'AssistantComposer owns the assistant-ui form shell around the controlled client-gui composer state',
    fallback: 'controlled form behavior remains inside AssistantComposer',
  },
  {
    primitive: 'ComposerPrimitive.Input',
    requiresProvider: true,
    canonicalStateRead: 'prompt remains controlled by ChatView/WelcomeView',
    actionForwardingTarget:
      'input/change/paste/submit keydown -> existing prompt and draft attachment handlers',
    chosenImplementation:
      'ComposerPrimitive.Input asChild over the controlled textarea with assistant-ui submission disabled',
    fallback: 'compatibility export ComposerSurface -> AssistantComposer',
  },
  {
    primitive: 'AttachmentPrimitive.Root',
    requiresProvider: true,
    canonicalStateRead: 'pastedImages and attachedFiles remain controlled by ChatView/WelcomeView',
    actionForwardingTarget: 'local remove image/file handlers',
    chosenImplementation:
      'AssistantAttachmentTray wraps each local draft attachment in AttachmentPrimitive.Root',
    fallback:
      'AttachmentPrimitive.Name/Remove stay unused until assistant-ui owns draft attachment state',
  },
  {
    primitive: 'ThreadPrimitive.Suggestion',
    requiresProvider: true,
    canonicalStateRead: 'WelcomeView quick tag prompt data',
    actionForwardingTarget: 'local prompt fill handler',
    chosenImplementation:
      'AssistantSuggestionList renders quick tags through ThreadPrimitive.Suggestion inside the runtime provider',
    fallback:
      'SuggestionPrimitive.Trigger is context-bound and not used for client-gui quick tag source data',
  },
  {
    primitive: 'ChainOfThoughtPrimitive.Root',
    requiresProvider: false,
    canonicalStateRead: 'thinking ContentBlock from client-gui messages',
    actionForwardingTarget: 'none; render-only reasoning adapter',
    chosenImplementation:
      'AssistantContentBlock wraps thinking blocks in ChainOfThoughtPrimitive.Root and delegates content to ThinkingBlock',
    fallback: 'ThinkingBlock remains the semantic renderer for preview and expansion behavior',
  },
];
