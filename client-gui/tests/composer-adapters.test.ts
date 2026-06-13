import { describe, expect, it } from 'vitest';
import {
  buildContentBlocksFromComposerData,
  buildFileAttachmentsFromFiles,
  buildFileAttachmentsFromPaths,
  buildImageDraftsFromFiles,
} from '../src/renderer/components/composer/composerAdapters';

const installMockFileReader = (result: string) => {
  const originalFileReader = globalThis.FileReader;

  class MockFileReader {
    result: string | ArrayBuffer | null = null;
    error: Error | null = null;
    onloadend: null | (() => void) = null;
    onerror: null | (() => void) = null;

    readAsDataURL() {
      this.result = `data:mock/type;base64,${result}`;
      this.onloadend?.();
    }
  }

  globalThis.FileReader = MockFileReader as unknown as typeof FileReader;

  return () => {
    globalThis.FileReader = originalFileReader;
  };
};

const installMockImageResizeEnvironment = () => {
  const originalImage = globalThis.Image;
  const originalDocument = globalThis.document;
  const originalCreateObjectURL = URL.createObjectURL;
  const originalRevokeObjectURL = URL.revokeObjectURL;

  class MockImage {
    width = 4000;
    height = 3000;
    onload: null | (() => void) = null;
    onerror: null | (() => void) = null;

    set src(_value: string) {
      this.onload?.();
    }
  }

  const mockCanvas = {
    width: 0,
    height: 0,
    getContext: () => ({
      clearRect: () => {},
      drawImage: () => {},
    }),
    toBlob: (callback: (blob: Blob | null) => void, type?: string) => {
      callback(new Blob(['compressed-image'], { type: type || 'image/png' }));
    },
  };

  globalThis.Image = MockImage as unknown as typeof Image;
  globalThis.document = {
    createElement: (tagName: string) => {
      if (tagName !== 'canvas') {
        throw new Error(`Unexpected element: ${tagName}`);
      }
      return mockCanvas as unknown as HTMLCanvasElement;
    },
  } as Document;
  URL.createObjectURL = (() => 'blob:resized-image') as typeof URL.createObjectURL;
  URL.revokeObjectURL = (() => {}) as typeof URL.revokeObjectURL;

  return () => {
    globalThis.Image = originalImage;
    globalThis.document = originalDocument;
    URL.createObjectURL = originalCreateObjectURL;
    URL.revokeObjectURL = originalRevokeObjectURL;
  };
};

describe('composer adapters', () => {
  it('builds a text-only payload when the composer only has prompt text', () => {
    expect(
      buildContentBlocksFromComposerData({
        prompt: '  hello assistant  ',
        pastedImages: [],
        attachedFiles: [],
      })
    ).toEqual([{ type: 'text', text: 'hello assistant' }]);
  });

  it('builds an image-only payload when the composer only has pasted images', () => {
    expect(
      buildContentBlocksFromComposerData({
        prompt: '   ',
        pastedImages: [
          {
            url: 'blob:one',
            base64: 'AAA',
            mediaType: 'image/png',
          },
        ],
        attachedFiles: [],
      })
    ).toEqual([
      {
        type: 'image',
        source: {
          type: 'base64',
          media_type: 'image/png',
          data: 'AAA',
        },
      },
    ]);
  });

  it('builds a file-only payload when the composer only has attachments', () => {
    expect(
      buildContentBlocksFromComposerData({
        prompt: '   ',
        pastedImages: [],
        attachedFiles: [
          {
            name: 'notes.txt',
            path: '/tmp/notes.txt',
            size: 42,
            type: 'text/plain',
            inlineDataBase64: 'QUJD',
          },
        ],
      })
    ).toEqual([
      {
        type: 'file_attachment',
        filename: 'notes.txt',
        relativePath: '/tmp/notes.txt',
        size: 42,
        mimeType: 'text/plain',
        inlineDataBase64: 'QUJD',
      },
    ]);
  });

  it('builds content blocks in the canonical order with trimmed prompt text', () => {
    const blocks = buildContentBlocksFromComposerData({
      prompt: '  hello assistant  ',
      pastedImages: [
        {
          url: 'blob:one',
          base64: 'AAA',
          mediaType: 'image/png',
        },
      ],
      attachedFiles: [
        {
          name: 'notes.txt',
          path: '/tmp/notes.txt',
          size: 42,
          type: 'text/plain',
          inlineDataBase64: 'QUJD',
        },
      ],
    });

    expect(blocks).toEqual([
      {
        type: 'image',
        source: {
          type: 'base64',
          media_type: 'image/png',
          data: 'AAA',
        },
      },
      {
        type: 'file_attachment',
        filename: 'notes.txt',
        relativePath: '/tmp/notes.txt',
        size: 42,
        mimeType: 'text/plain',
        inlineDataBase64: 'QUJD',
      },
      {
        type: 'text',
        text: 'hello assistant',
      },
    ]);
  });

  it('returns an empty payload when the composer is blank', () => {
    expect(
      buildContentBlocksFromComposerData({
        prompt: '   ',
        pastedImages: [],
        attachedFiles: [],
      })
    ).toEqual([]);
  });

  it('maps file paths to stable attachment metadata', () => {
    expect(buildFileAttachmentsFromPaths(['/workspace/a.md', 'C:\\temp\\b.bin'])).toEqual([
      {
        name: 'a.md',
        path: '/workspace/a.md',
        size: 0,
        type: 'application/octet-stream',
      },
      {
        name: 'b.bin',
        path: 'C:\\temp\\b.bin',
        size: 0,
        type: 'application/octet-stream',
      },
    ]);
  });

  it('builds image drafts from image files with preserved media type and base64 data', async () => {
    const restoreFileReader = installMockFileReader('SU1BR0U=');
    const originalCreateObjectURL = URL.createObjectURL;
    const originalRevokeObjectURL = URL.revokeObjectURL;

    URL.createObjectURL = undefined as unknown as typeof URL.createObjectURL;
    URL.revokeObjectURL = (() => {}) as typeof URL.revokeObjectURL;

    try {
      const files = [new File([new Uint8Array([1, 2, 3])], 'photo.png', { type: 'image/png' })];
      const drafts = await buildImageDraftsFromFiles(files);

      expect(drafts).toHaveLength(1);
      expect(drafts[0]).toMatchObject({
        base64: 'SU1BR0U=',
        mediaType: 'image/png',
        url: 'data:image/png;base64,SU1BR0U=',
      });
    } finally {
      restoreFileReader();
      URL.createObjectURL = originalCreateObjectURL;
      URL.revokeObjectURL = originalRevokeObjectURL;
    }
  });

  it('resizes oversized image files before converting them to base64 drafts', async () => {
    const restoreFileReader = installMockFileReader('Q09NUFJFU1NFRA==');
    const restoreImageEnvironment = installMockImageResizeEnvironment();

    try {
      const oversizedBytes = new Uint8Array(4 * 1024 * 1024 + 1);
      const files = [new File([oversizedBytes], 'large.png', { type: 'image/png' })];
      const drafts = await buildImageDraftsFromFiles(files);

      expect(drafts).toHaveLength(1);
      expect(drafts[0]).toMatchObject({
        base64: 'Q09NUFJFU1NFRA==',
        mediaType: 'image/png',
        url: 'blob:resized-image',
      });
    } finally {
      restoreImageEnvironment();
      restoreFileReader();
    }
  });

  it('builds file attachments with inline data for in-memory files and stable metadata for dropped paths', async () => {
    const restoreFileReader = installMockFileReader('RklMRQ==');

    try {
      const inlineFile = new File([new Uint8Array([4, 5, 6])], 'report.txt', {
        type: 'text/plain',
      });
      const droppedFile = new File([new Uint8Array([7, 8, 9])], 'bundle.bin', {
        type: 'application/octet-stream',
      }) as File & { path?: string };
      Object.defineProperty(droppedFile, 'path', {
        value: '/tmp/bundle.bin',
      });

      const attachments = await buildFileAttachmentsFromFiles([inlineFile, droppedFile]);

      expect(attachments).toEqual([
        {
          name: 'report.txt',
          path: '',
          size: inlineFile.size,
          type: 'text/plain',
          inlineDataBase64: 'RklMRQ==',
        },
        {
          name: 'bundle.bin',
          path: '/tmp/bundle.bin',
          size: droppedFile.size,
          type: 'application/octet-stream',
          inlineDataBase64: undefined,
        },
      ]);
    } finally {
      restoreFileReader();
    }
  });
});
