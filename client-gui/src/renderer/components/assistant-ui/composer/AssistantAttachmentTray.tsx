import { AttachmentPrimitive } from '@assistant-ui/react';
import { X } from 'lucide-react';
import type { ComposerFileAttachment, ComposerImage } from '../../composer/composerAdapters';

interface AssistantAttachmentTrayProps {
  pastedImages: ComposerImage[];
  attachedFiles: ComposerFileAttachment[];
  onRemoveImage: (index: number) => void;
  onRemoveFile: (index: number) => void;
  getPastedImageAlt: (index: number) => string;
  fileListClassName?: string;
}

function getImageTileClassName(count: number): string {
  return `grid grid-cols-2 ${count > 2 ? 'sm:grid-cols-3' : ''} ${count > 3 ? 'md:grid-cols-4' : ''} ${count > 4 ? 'lg:grid-cols-5' : ''} gap-2`;
}

export function AssistantAttachmentTray({
  pastedImages,
  attachedFiles,
  onRemoveImage,
  onRemoveFile,
  getPastedImageAlt,
  fileListClassName,
}: AssistantAttachmentTrayProps): JSX.Element {
  return (
    <>
      {pastedImages.length > 0 && (
        <div
          className={`${getImageTileClassName(pastedImages.length)} mb-3`}
          data-assistant-attachment-kind="image-drafts"
        >
          {pastedImages.map((img, index) => (
            <AttachmentPrimitive.Root
              key={img.url || `pasted-image-${index}`}
              className="relative group"
              data-assistant-attachment-kind="image"
            >
              <img
                src={img.url}
                alt={getPastedImageAlt(index)}
                className="w-full aspect-square object-cover rounded-lg border border-border block"
              />
              <button
                type="button"
                onClick={() => onRemoveImage(index)}
                className="absolute -top-1 -right-1 w-5 h-5 rounded-full bg-error text-white flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity"
                aria-label={`Remove pasted image ${index + 1}`}
              >
                <X className="w-3 h-3" />
              </button>
            </AttachmentPrimitive.Root>
          ))}
        </div>
      )}

      {attachedFiles.length > 0 && (
        <div
          className={`space-y-2 mb-3 ${fileListClassName ?? ''}`}
          data-assistant-attachment-kind="file-drafts"
        >
          {attachedFiles.map((file, index) => (
            <AttachmentPrimitive.Root
              key={file.path || `attached-file-${index}`}
              className="flex items-center gap-2 px-3 py-2 rounded-lg bg-surface-muted border border-border group"
              data-assistant-attachment-kind="file"
            >
              <div className="flex-1 min-w-0">
                <p className="text-sm text-text-primary truncate">{file.name}</p>
              </div>
              <button
                type="button"
                onClick={() => onRemoveFile(index)}
                className="w-6 h-6 rounded-full bg-error/10 hover:bg-error/20 text-error flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity"
                aria-label={`Remove attached file ${file.name}`}
              >
                <X className="w-3.5 h-3.5" />
              </button>
            </AttachmentPrimitive.Root>
          ))}
        </div>
      )}
    </>
  );
}
