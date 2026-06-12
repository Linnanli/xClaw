import { useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Check, ChevronDown, ChevronRight, CirclePlus, Sparkles, Settings2 } from 'lucide-react';
import { useClientGuiModelSelector } from '../../hooks/useClientGuiModelSelector';

export interface AssistantModelOption {
  modelId: string;
  displayName: string;
  isDefault: boolean;
}

export type AssistantModelSelectorMode = 'loading' | 'server' | 'config' | 'unavailable';

export const normalizeServerModelOptions = (
  config: ClientModelProviderConfig
): AssistantModelOption[] =>
  config.models.map((model, index) => ({
    modelId: model.modelId,
    displayName: model.displayName || model.modelId,
    isDefault: model.isDefault || index === 0,
  }));

export const normalizeConfigModelOptions = (models: ProviderModelInfo[]): AssistantModelOption[] =>
  models.map((model, index) => ({
    modelId: model.id,
    displayName: model.name || model.id,
    isDefault: index === 0,
  }));

export function resolveModelDisplayName(
  selectedModelId: string,
  options: AssistantModelOption[],
  fallbackLabel: string
): string {
  const selected = options.find((model) => model.modelId === selectedModelId);
  if (selected?.displayName) {
    return selected.displayName;
  }
  return selectedModelId || fallbackLabel;
}

export function AssistantModelSelector(): JSX.Element {
  const { t } = useTranslation();
  const { models, selectedModelId, loading, error, selectModel, openSettings } =
    useClientGuiModelSelector();

  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const handlePointerDown = (event: MouseEvent) => {
      if (rootRef.current && !rootRef.current.contains(event.target as Node)) {
        setOpen(false);
      }
    };

    document.addEventListener('mousedown', handlePointerDown);
    return () => document.removeEventListener('mousedown', handlePointerDown);
  }, [open]);

  const selectedModelLabel = resolveModelDisplayName(
    selectedModelId,
    models,
    t('chat.noModel', 'No model')
  );

  if (loading && models.length === 0) {
    return (
      <div
        className="inline-flex h-9 items-center gap-1.5 rounded-2xl border border-border-subtle bg-background/60 px-3 text-xs text-text-muted"
        aria-busy="true"
        aria-label={t('chat.loadingModels', 'Loading models')}
      >
        <Sparkles className="h-3.5 w-3.5 animate-pulse text-accent" />
        <span>{t('chat.loadingModels', 'Loading models')}</span>
      </div>
    );
  }

  return (
    <div ref={rootRef} className="relative">
      <button
        type="button"
        onClick={() => setOpen((prev) => !prev)}
        className="inline-flex h-9 max-w-[16rem] items-center gap-1.5 rounded-2xl border border-border-subtle bg-background/70 px-3 text-xs font-medium text-text-secondary transition-colors hover:bg-surface-hover hover:text-text-primary"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-label={`${t('chat.currentModel', 'Current model')}: ${selectedModelLabel}`}
        data-testid="model-selector-trigger"
      >
        <Sparkles className="h-3.5 w-3.5 shrink-0 text-accent" />
        <span className="truncate">{selectedModelLabel}</span>
        <ChevronDown
          className={`h-3.5 w-3.5 shrink-0 transition-transform ${open ? 'rotate-180' : ''}`}
        />
      </button>

      {open && (
        <div
          role="listbox"
          aria-label={t('chat.selectModel', 'Choose model')}
          className="absolute bottom-full right-0 z-50 mb-2 w-64 overflow-hidden rounded-2xl border border-border bg-background p-1.5 shadow-[0_8px_24px_rgba(0,0,0,0.12)]"
          data-testid="model-selector-panel"
        >
          <div className="px-3 py-1.5">
            <p className="text-[11px] font-semibold uppercase tracking-[0.08em] text-text-muted">
              {t('chat.selectModel', 'Choose model')}
            </p>
          </div>

          <div className="space-y-1">
            {models.map((model) => {
              const selected = model.modelId === selectedModelId;
              return (
                <button
                  key={model.modelId}
                  type="button"
                  role="option"
                  aria-selected={selected}
                  onClick={() => {
                    void selectModel(model.modelId);
                    setOpen(false);
                  }}
                  className={`flex w-full items-center justify-between rounded-xl px-3 py-2 text-left text-sm transition-colors ${
                    selected ? 'bg-accent-muted text-accent' : 'hover:bg-surface-hover text-text-primary'
                  }`}
                  data-testid={`model-option-${model.modelId}`}
                >
                  <div className="flex min-w-0 items-center gap-2">
                    <Sparkles
                      className={`h-3.5 w-3.5 shrink-0 ${selected ? 'text-accent' : 'text-text-muted'}`}
                    />
                    <span className={`truncate ${selected ? 'font-semibold' : 'font-medium'}`}>
                      {model.displayName}
                    </span>
                    {model.isDefault && (
                      <span className="shrink-0 rounded-full bg-green-200 px-1.5 py-0.5 text-[10px] font-medium text-green-700 dark:bg-green-500/20 dark:text-green-300">
                        {t('chat.recommended', 'Recommended')}
                      </span>
                    )}
                  </div>
                  {selected && <Check className="h-3.5 w-3.5 shrink-0 text-accent" />}
                </button>
              );
            })}
          </div>

          <div className="my-1 h-px bg-border-muted" />

          <button
            type="button"
            onClick={openSettings}
            className="flex w-full items-center justify-between rounded-xl px-3 py-2 text-left text-sm text-text-primary transition-colors hover:bg-surface-hover"
            data-testid="model-selector-settings"
          >
            <div className="flex items-center gap-2">
              <CirclePlus className="h-3.5 w-3.5 shrink-0 text-text-muted" />
              <span className="font-medium">{t('chat.openSettings', 'Open settings')}</span>
            </div>
            <ChevronRight className="h-3.5 w-3.5 shrink-0 text-text-muted" />
          </button>

          {(error || models.length === 0) && (
            <div className="px-3 py-2 text-[11px] leading-relaxed text-text-muted">
              <div className="flex items-center gap-1.5">
                <Settings2 className="h-3 w-3 shrink-0" />
                <span>{error || t('chat.noModelsAvailable', 'No models available')}</span>
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
