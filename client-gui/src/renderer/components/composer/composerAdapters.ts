import type { ContentBlock } from '../../types';

export type ComposerImageMediaType = 'image/jpeg' | 'image/png' | 'image/gif' | 'image/webp';

export interface ComposerImage {
  url: string;
  base64: string;
  mediaType: ComposerImageMediaType;
}

export interface ComposerFileAttachment {
  name: string;
  path: string;
  size: number;
  type: string;
  inlineDataBase64?: string;
}

export interface ComposerContentInput {
  prompt: string;
  pastedImages: ComposerImage[];
  attachedFiles: ComposerFileAttachment[];
}

export interface ComposerImageErrorContext {
  source: 'paste' | 'drop';
}

const MAX_BLOB_SIZE = 3.75 * 1024 * 1024;

const normalizeImageMediaType = (mediaType: string): ComposerImageMediaType => {
  if (mediaType === 'image/png' || mediaType === 'image/gif' || mediaType === 'image/webp') {
    return mediaType;
  }

  return 'image/jpeg';
};

export const buildContentBlocksFromComposerData = ({
  prompt,
  pastedImages,
  attachedFiles,
}: ComposerContentInput): ContentBlock[] => {
  const contentBlocks: ContentBlock[] = [];

  pastedImages.forEach((img) => {
    contentBlocks.push({
      type: 'image',
      source: {
        type: 'base64',
        media_type: img.mediaType,
        data: img.base64,
      },
    });
  });

  attachedFiles.forEach((file) => {
    contentBlocks.push({
      type: 'file_attachment',
      filename: file.name,
      relativePath: file.path,
      size: file.size,
      mimeType: file.type,
      inlineDataBase64: file.inlineDataBase64,
    });
  });

  const trimmedPrompt = prompt.trim();
  if (trimmedPrompt) {
    contentBlocks.push({
      type: 'text',
      text: trimmedPrompt,
    });
  }

  return contentBlocks;
};

export const blobToBase64 = (blob: Blob): Promise<string> => {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();

    reader.onloadend = () => {
      const result = reader.result;
      if (typeof result !== 'string') {
        reject(new Error('FileReader result is not a string'));
        return;
      }

      const parts = result.split(',');
      resolve(parts[1] || '');
    };

    reader.onerror = () => {
      reject(new Error(reader.error?.message || 'Failed to read blob'));
    };

    reader.readAsDataURL(blob);
  });
};

const resizeImageBlobIfNeeded = async (blob: Blob): Promise<Blob> => {
  if (blob.size <= MAX_BLOB_SIZE) {
    return blob;
  }

  return new Promise((resolve, reject) => {
    const img = new Image();
    const url = URL.createObjectURL(blob);

    img.onload = () => {
      URL.revokeObjectURL(url);

      const scale = Math.sqrt(MAX_BLOB_SIZE / blob.size);
      const quality = 0.9;
      const canvas = document.createElement('canvas');
      const ctx = canvas.getContext('2d');

      if (!ctx) {
        reject(new Error('Failed to get canvas context'));
        return;
      }

      const attemptCompress = (
        currentScale: number,
        currentQuality: number,
      ): Promise<Blob> => {
        canvas.width = Math.max(1, Math.floor(img.width * currentScale));
        canvas.height = Math.max(1, Math.floor(img.height * currentScale));

        ctx.clearRect(0, 0, canvas.width, canvas.height);
        ctx.drawImage(img, 0, 0, canvas.width, canvas.height);

        return new Promise((resolveBlob) => {
          canvas.toBlob(
            (compressedBlob) => {
              if (!compressedBlob) {
                reject(new Error('Failed to compress image'));
                return;
              }

              if (
                compressedBlob.size > MAX_BLOB_SIZE &&
                (currentQuality > 0.5 || currentScale > 0.3)
              ) {
                const newQuality = Math.max(0.5, currentQuality - 0.1);
                const newScale = currentQuality <= 0.5 ? currentScale * 0.9 : currentScale;
                attemptCompress(newScale, newQuality).then(resolveBlob);
              } else {
                resolveBlob(compressedBlob);
              }
            },
            blob.type || 'image/jpeg',
            currentQuality
          );
        });
      };

      attemptCompress(scale, quality).then(resolve).catch(reject);
    };

    img.onerror = () => {
      URL.revokeObjectURL(url);
      reject(new Error('Failed to load image'));
    };

    img.src = url;
  });
};

const buildComposerImageFromBlob = async (blob: Blob): Promise<ComposerImage> => {
  const resizedBlob = await resizeImageBlobIfNeeded(blob);
  const base64 = await blobToBase64(resizedBlob);
  const safeMediaType = normalizeImageMediaType(resizedBlob.type || 'image/jpeg');
  const url =
    typeof URL !== 'undefined' && typeof URL.createObjectURL === 'function'
      ? URL.createObjectURL(resizedBlob)
      : `data:${safeMediaType};base64,${base64}`;

  return {
    url,
    base64,
    mediaType: safeMediaType,
  };
};

export const buildImageDraftsFromClipboardItems = async (
  items: DataTransferItemList | null | undefined,
): Promise<ComposerImage[]> => {
  if (!items) {
    return [];
  }

  const imageItems = Array.from(items).filter((item) => item.type.startsWith('image/'));
  const newImages: ComposerImage[] = [];

  for (const item of imageItems) {
    const file = item.getAsFile();
    if (!file) {
      continue;
    }

    newImages.push(await buildComposerImageFromBlob(file));
  }

  return newImages;
};

export const buildImageDraftsFromFiles = async (
  files: File[],
): Promise<ComposerImage[]> => {
  const imageFiles = files.filter((file) => file.type.startsWith('image/'));
  const newImages: ComposerImage[] = [];

  for (const file of imageFiles) {
    newImages.push(await buildComposerImageFromBlob(file));
  }

  return newImages;
};

export const buildFileAttachmentsFromFiles = async (
  files: File[],
): Promise<ComposerFileAttachment[]> => {
  const nonImageFiles = files.filter((file) => !file.type.startsWith('image/'));
  return Promise.all(
    nonImageFiles.map(async (file) => {
      const droppedPath = 'path' in file && typeof file.path === 'string' ? file.path : '';
      const inlineDataBase64 = droppedPath ? undefined : await blobToBase64(file);

      return {
        name: file.name,
        path: droppedPath,
        size: file.size,
        type: file.type || 'application/octet-stream',
        inlineDataBase64,
      } as ComposerFileAttachment;
    })
  );
};

export const buildFileAttachmentsFromPaths = (filePaths: string[]): ComposerFileAttachment[] => {
  return filePaths.map((filePath) => {
    const fileName = filePath.split(/[/\\]/).pop() || 'unknown';

    return {
      name: fileName,
      path: filePath,
      size: 0,
      type: 'application/octet-stream',
    };
  });
};
