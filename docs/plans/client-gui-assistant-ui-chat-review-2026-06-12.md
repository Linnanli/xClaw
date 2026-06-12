# client-gui assistant-ui chat replacement review

Date: 2026-06-12
Scope: review/docs note migrated from `.omx/plans` into a tracked repository document.

## What is already in place

- `client-gui/src/main/dasclaw/app-server-session-bridge.ts`
  - backend model-selection bridge and queuing logic exist
  - `selectModelForNextTurn()` persists the chosen model and forwards it to the app-server path
- `client-gui/tests/dasclaw-app-server-session-bridge.test.ts`
  - targeted bridge tests pass, including model-selection coverage
- `client-gui/src/renderer/components/assistant-ui/*`
  - adapter-shell files exist for thread/message/content rendering

## What remains open

- `client-gui/src/renderer/components/ChatView.tsx`
  - still renders the legacy chat stack
  - still shows a static model chip instead of a real selector
- `client-gui/src/renderer/components/WelcomeView.tsx`
  - still owns duplicated composer/attachment handling
- `client-gui/src/renderer/hooks/useIPC.ts`
  - no committed-state handler for `modelProvider.changed`
- `client-gui/src/renderer/store/index.ts`
  - no canonical renderer slice for model-provider committed selection
- `client-gui/src/renderer/components/assistant-ui/*`
  - compatibility wrappers only; no primitive adoption or renderer integration yet

## Verification completed during review

- `npm --prefix client-gui run test -- --run tests/dasclaw-app-server-session-bridge.test.ts` → PASS
- `npm --prefix client-gui run typecheck` → PASS
- `cd client-gui && ./node_modules/.bin/eslint src/main/dasclaw/app-server-session-bridge.ts src/renderer/components/WelcomeView.tsx src/renderer/components/assistant-ui/**/*.ts src/renderer/components/assistant-ui/**/*.tsx` → PASS
- `npm --prefix client-gui run lint` → FAIL on pre-existing unrelated repo issues
- `npm --prefix client-gui run test -- --run` → FAIL on pre-existing unrelated repo issues / missing native bindings in this environment

## Risk / next step

The backend bridge is ahead of the renderer. The next safe step is to wire the renderer’s model selector and the ChatView/WelcomeView assistant-ui shell into the existing IPC/store contracts without broadening scope into unrelated shared implementation files.
