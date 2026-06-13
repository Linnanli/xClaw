import { describe, expect, it } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';
import { assistantPrimitiveCoverage } from '../src/renderer/components/assistant-ui/primitiveCoverage';

describe('assistant-ui renderer contract', () => {
  it('declares @assistant-ui/react as a real client-gui dependency', () => {
    const packageJson = JSON.parse(
      fs.readFileSync(path.resolve(process.cwd(), 'package.json'), 'utf8')
    ) as { dependencies?: Record<string, string> };

    expect(packageJson.dependencies?.['@assistant-ui/react']).toBe('0.12.28');
  });

  it('keeps the primitive coverage matrix focused on the adopted assistant-ui surfaces', () => {
    const coveredPrimitives = assistantPrimitiveCoverage.map((row) => row.primitive);

    expect(coveredPrimitives).toEqual(
      expect.arrayContaining([
        'AssistantRuntimeProvider + useExternalStoreRuntime',
        'ThreadPrimitive.Root + ThreadPrimitive.Viewport + ThreadPrimitive.Messages',
        'MessagePrimitive.Root',
        'ActionBarPrimitive.Root + ActionBarPrimitive.Copy',
        'ComposerPrimitive.Root',
        'ComposerPrimitive.Input',
        'AttachmentPrimitive.Root',
        'ThreadPrimitive.Suggestion',
        'ChainOfThoughtPrimitive.Root',
      ])
    );
  });

  it('documents adapter ownership instead of hiding fallback state', () => {
    expect(
      assistantPrimitiveCoverage.find((row) => row.primitive === 'ComposerPrimitive.Input')
        ?.canonicalStateRead
    ).toContain('prompt remains controlled by ChatView/WelcomeView');

    expect(
      assistantPrimitiveCoverage.find((row) => row.primitive === 'AttachmentPrimitive.Root')
        ?.fallback
    ).toContain('AttachmentPrimitive.Name/Remove stay unused');

    expect(
      assistantPrimitiveCoverage.find((row) => row.primitive === 'ThreadPrimitive.Suggestion')
        ?.chosenImplementation
    ).toContain('AssistantSuggestionList');
  });
});
