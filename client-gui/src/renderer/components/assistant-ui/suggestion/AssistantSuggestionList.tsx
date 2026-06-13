import { ThreadPrimitive } from '@assistant-ui/react';
import type { ComponentType } from 'react';

export interface AssistantSuggestionItem {
  id: string;
  label: string;
  prompt: string;
  icon: ComponentType<{ className?: string }>;
  badge?: string;
}

interface AssistantSuggestionListProps {
  suggestions: AssistantSuggestionItem[];
  selectedId: string | null;
  onSelect: (id: string, prompt: string) => void;
}

export function AssistantSuggestionList({
  suggestions,
  selectedId,
  onSelect,
}: AssistantSuggestionListProps): JSX.Element {
  return (
    <div className="flex flex-wrap gap-2 justify-center px-3" data-assistant-suggestions>
      {suggestions.map((suggestion) => {
        const Icon = suggestion.icon;
        const selected = selectedId === suggestion.id;

        return (
          <ThreadPrimitive.Suggestion
            key={suggestion.id}
            prompt={suggestion.prompt}
            clearComposer
            type="button"
            onClick={(event) => {
              event.preventDefault();
              onSelect(suggestion.id, suggestion.prompt);
            }}
            className={`inline-flex items-center gap-2 rounded-full border px-3 py-2 text-sm transition-colors ${
              selected
                ? 'border-accent/30 bg-accent-muted text-accent'
                : 'border-border-subtle bg-background/65 text-text-secondary hover:bg-surface-hover hover:text-text-primary'
            } ${suggestion.badge ? 'relative' : ''}`}
            data-assistant-suggestion-id={suggestion.id}
          >
            <Icon className={`w-4 h-4 ${selected ? 'text-accent' : 'text-text-muted'}`} />
            <span>{suggestion.label}</span>
            {suggestion.badge && (
              <span className="ml-1 px-1.5 py-px text-[9px] rounded bg-surface-active text-text-muted">
                {suggestion.badge}
              </span>
            )}
          </ThreadPrimitive.Suggestion>
        );
      })}
    </div>
  );
}
