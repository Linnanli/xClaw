# client-gui assistant-ui chat replacement review

_Reviewed: 2026-06-12_

This note captures the current quality/risk snapshot for the `client-gui` assistant-ui migration baseline that follows commit `4fd39ae73aaf9844583ea583f9b9de2d247f95d1`.

## What is already in place

- `client-gui/src/renderer/components/assistant-ui/AssistantModelSelector.tsx`
  - The selector is no longer a static label; it renders a real model picker UI.
  - It delegates all model state to `useClientGuiModelSelector()`.
- `client-gui/src/renderer/hooks/useClientGuiModelSelector.ts`
  - The renderer bridge now treats `modelProvider.changed` as committed state.
  - It preserves both selection paths:
    - `modelProvider.selectForNextTurn` for app-server mode
    - `config.listModels` / `config.save` for config-backed mode
- `client-gui/src/renderer/components/ChatView.tsx`
  - The chat shell still owns the legacy composer/streaming layout.
  - The assistant-ui selector is wired into the composer row, so model switching is no longer a static chip.

## Code-quality observations

- The selector bridge is intentionally conservative and fails closed.
  - If the bridge cannot list models, it surfaces an error instead of inventing a fake selection state.
- The selector component is presentational.
  - The persistence and commit logic stays in the hook, which keeps the UI shell smaller and easier to review.
- The chat shell is still hybrid.
  - `ChatView.tsx` still carries the streaming insertion logic, stop button, attachment tray, and legacy message rendering.
  - That is acceptable for the current stage, but it keeps a lot of behavior concentrated in one component.

## Risks to keep watching

1. **Thread wiring remains incomplete**
   - The assistant-ui selector has landed, but the chat thread still relies on the legacy `MessageCard` path.
   - Risk: the new selector can survive while the intended assistant-ui thread migration stalls.

2. **Regression coverage is still mostly source-contract based**
   - The new tests verify key strings and hook contracts, which is useful for a migration spike.
   - Risk: we still need a render-level check for streaming insertion and stop behavior to prove the shell behaves end-to-end.

3. **Model-provider bridge depends on renderer event propagation**
   - `modelProvider.changed` must keep reaching the renderer store for the selector to stay truthful.
   - Risk: if that event wiring regresses, the selector can drift from the committed model.

4. **Baseline test-suite failures are unrelated but noisy**
   - The full client-gui suite still shows environment/baseline failures in memory and pre-build checks in this worktree.
   - Risk: those unrelated failures can hide genuine regressions unless the focused assistant-ui tests remain green.

## Recommended next steps

- Keep the bridge tests for `modelProvider.changed` and selector persistence.
- Add one render-level regression for chat streaming insertion or stop behavior.
- Continue the assistant-ui thread/composer wiring in the implementation lane without rewriting already-completed selector work.

