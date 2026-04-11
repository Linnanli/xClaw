# React and TypeScript Review Guide

## Scope

Use for `.ts`, `.tsx`, `.js`, `.jsx` changes. Focus on behavior regressions over style.

## React lifecycle and state

- Check `useEffect` cleanup for listeners, timers, subscriptions, and in-flight requests.
- Check dependency arrays for stale closure bugs.
- Check for state updates after unmount and cancelled async paths.
- Check derived state anti-patterns (`useEffect` computing state from props repeatedly).
- Check optimistic UI rollback correctness after failed mutation.

## Event ordering and UI behavior

- Check race between concurrent requests (slow response overwriting newer state).
- Check idempotency of event handlers under double click or retry.
- Check tab/thread switch cleanup (old async results must not patch new context).
- Check modal/dialog open-close state resets.

## TypeScript safety

- Prefer precise types over `any` and wide `unknown` assertions.
- Check nullable paths (`null` and `undefined`) at boundaries.
- Check discriminated unions for exhaustive handling.
- Check API response parsing and validation before use.

## Data and performance

- Check query key completeness for cache libraries.
- Check missing invalidation after write operations.
- Check unbounded list rendering and missing pagination/virtualization.

## Tests to request

- Lifecycle cleanup test (unmount and switch context).
- Race test (out-of-order response handling).
- Error-path test (rollback, retries, fallback UI).
