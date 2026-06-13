import { useEffect, useRef } from 'react';
import { ComposerPrimitive } from '@assistant-ui/react';
import type {
  CSSProperties,
  ClipboardEvent,
  DragEvent,
  FormEvent,
  ReactNode,
  RefObject,
} from 'react';
import { AssistantAttachmentTray } from './AssistantAttachmentTray';
import type { ComposerFileAttachment, ComposerImage } from '../../composer/composerAdapters';

export interface AssistantComposerProps {
  prompt: string;
  onPromptChange: (value: string) => void;
  textareaRef: RefObject<HTMLTextAreaElement>;
  isSubmitting: boolean;
  pastedImages: ComposerImage[];
  attachedFiles: ComposerFileAttachment[];
  onRemoveImage: (index: number) => void;
  onRemoveFile: (index: number) => void;
  onPaste: (e: ClipboardEvent<HTMLTextAreaElement>) => void;
  onSubmit: () => Promise<void> | void;
  placeholder: string;
  attachmentImageAlt: (index: number) => string;
  onDragOver?: (e: DragEvent<HTMLFormElement>) => void;
  onDragLeave?: (e: DragEvent<HTMLFormElement>) => void;
  onDrop?: (e: DragEvent<HTMLFormElement>) => void;
  leftActions?: ReactNode;
  rightActions?: ReactNode;
  footer?: ReactNode;
  layout?: 'inline' | 'stacked';
  composerClassName?: string;
  trayClassName?: string;
  textareaClassName?: string;
  textareaStyle?: CSSProperties;
  textareaMinHeight?: number;
  textareaMaxHeight?: number;
}

interface ComposerKeydownEventState {
  key: string;
  shiftKey: boolean;
  isComposing: boolean;
  keyCode?: number;
}

interface ComposerTextareaLike {
  scrollHeight: number;
  style: {
    height: string;
    overflowY: string;
  };
}

export function shouldSubmitComposerOnKeyDown({
  key,
  shiftKey,
  isComposing,
  keyCode,
}: ComposerKeydownEventState): boolean {
  return key === 'Enter' && !shiftKey && !isComposing && keyCode !== 229;
}

export function resizeComposerTextarea(
  textarea: ComposerTextareaLike,
  minHeight: number,
  maxHeight: number
): void {
  textarea.style.height = 'auto';
  const nextHeight = Math.min(textarea.scrollHeight, maxHeight);
  textarea.style.height = `${Math.max(minHeight, nextHeight)}px`;
  textarea.style.overflowY = textarea.scrollHeight > maxHeight ? 'auto' : 'hidden';
}

const defaultTextareaMinHeight = 72;
const defaultTextareaMaxHeight = 200;

export function AssistantComposer({
  prompt,
  onPromptChange,
  textareaRef,
  isSubmitting,
  pastedImages,
  attachedFiles,
  onRemoveImage,
  onRemoveFile,
  onPaste,
  onSubmit,
  placeholder,
  attachmentImageAlt,
  onDragOver,
  onDragLeave,
  onDrop,
  leftActions,
  rightActions,
  footer,
  layout = 'inline',
  composerClassName,
  trayClassName,
  textareaClassName,
  textareaStyle,
  textareaMinHeight = defaultTextareaMinHeight,
  textareaMaxHeight = defaultTextareaMaxHeight,
}: AssistantComposerProps): JSX.Element {
  const isComposingRef = useRef(false);
  const isStackedLayout = layout === 'stacked';

  useEffect(() => {
    const textarea = textareaRef.current;
    if (!textarea) {
      return;
    }

    resizeComposerTextarea(textarea, textareaMinHeight, textareaMaxHeight);
  }, [prompt, textareaMaxHeight, textareaMinHeight, textareaRef]);

  const handleFormSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    await onSubmit();
  };

  const inputNode = (
    <ComposerPrimitive.Input
      asChild
      value={prompt}
      onChange={(event) => onPromptChange(event.target.value)}
      onCompositionStart={() => {
        isComposingRef.current = true;
      }}
      onCompositionEnd={() => {
        isComposingRef.current = false;
      }}
      onPaste={onPaste}
      onKeyDown={(event) => {
        if (
          shouldSubmitComposerOnKeyDown({
            key: event.key,
            shiftKey: event.shiftKey,
            isComposing: event.nativeEvent.isComposing || isComposingRef.current,
            keyCode: event.keyCode,
          })
        ) {
          event.preventDefault();
          void onSubmit();
        }
      }}
      placeholder={placeholder}
      disabled={isSubmitting}
      submitMode="none"
      addAttachmentOnPaste={false}
      unstable_focusOnRunStart={false}
    >
      <textarea ref={textareaRef} rows={1} style={textareaStyle} className={textareaClassName} />
    </ComposerPrimitive.Input>
  );

  return (
    <ComposerPrimitive.Root
      onSubmit={handleFormSubmit}
      onDragOver={onDragOver}
      onDragLeave={onDragLeave}
      onDrop={onDrop}
      className={composerClassName}
      data-assistant-composer="client-gui"
    >
      <AssistantAttachmentTray
        pastedImages={pastedImages}
        attachedFiles={attachedFiles}
        onRemoveImage={onRemoveImage}
        onRemoveFile={onRemoveFile}
        getPastedImageAlt={attachmentImageAlt}
        fileListClassName={trayClassName}
      />

      {isStackedLayout ? (
        <div className="space-y-4">
          {inputNode}
          <div className="flex items-center justify-between gap-3">
            {leftActions}
            {rightActions}
          </div>
        </div>
      ) : (
        <div className="flex items-end gap-2">
          {leftActions}
          {inputNode}
          {rightActions}
        </div>
      )}

      {footer}
    </ComposerPrimitive.Root>
  );
}
