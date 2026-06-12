import { describe, expect, it } from 'vitest';
import {
  buildContentBlocksFromComposerData,
  buildFileAttachmentsFromPaths,
} from '../src/renderer/components/composer/composerAdapters';

describe('composer adapters', () => {
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
      }),
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
});
