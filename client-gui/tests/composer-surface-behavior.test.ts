import { describe, expect, it } from 'vitest';
import {
  resizeComposerTextarea,
  shouldSubmitComposerOnKeyDown,
} from '../src/renderer/components/composer/ComposerSurface';

describe('ComposerSurface behavior helpers', () => {
  it('submits only on bare Enter and respects IME/Shift modifiers', () => {
    expect(
      shouldSubmitComposerOnKeyDown({
        key: 'Enter',
        shiftKey: false,
        isComposing: false,
        keyCode: 13,
      })
    ).toBe(true);

    expect(
      shouldSubmitComposerOnKeyDown({
        key: 'Enter',
        shiftKey: true,
        isComposing: false,
        keyCode: 13,
      })
    ).toBe(false);

    expect(
      shouldSubmitComposerOnKeyDown({
        key: 'Enter',
        shiftKey: false,
        isComposing: true,
        keyCode: 13,
      })
    ).toBe(false);

    expect(
      shouldSubmitComposerOnKeyDown({
        key: 'Enter',
        shiftKey: false,
        isComposing: false,
        keyCode: 229,
      })
    ).toBe(false);
  });

  it('resizes within the configured min/max bounds and exposes overflow intent', () => {
    const textarea = {
      scrollHeight: 56,
      style: {
        height: '120px',
        overflowY: 'auto',
      },
    };

    resizeComposerTextarea(textarea, 72, 200);

    expect(textarea.style.height).toBe('72px');
    expect(textarea.style.overflowY).toBe('hidden');

    textarea.scrollHeight = 340;
    resizeComposerTextarea(textarea, 72, 200);

    expect(textarea.style.height).toBe('200px');
    expect(textarea.style.overflowY).toBe('auto');
  });
});
