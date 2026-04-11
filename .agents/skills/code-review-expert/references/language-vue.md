# Vue Review Guide

## Scope

Use for `.vue` and Vue-related composables/stores.

## Reactivity correctness

- Check `ref` vs `reactive` usage and accidental deep mutation.
- Check watcher source correctness and immediate/deep options.
- Check cleanup in watchers (`onInvalidate`) for async side effects.
- Check computed purity (no side effects inside computed).

## Lifecycle and behavior

- Check `onMounted` and `onUnmounted` symmetry for listeners and intervals.
- Check stale async result writes after route/view switch.
- Check event ordering for emits and parent state updates.
- Check dialog/form reset logic when component is reused.

## Composition and state

- Check composables for hidden shared mutable state.
- Check Pinia/Vuex actions for atomic state transitions on failure.
- Check error handling paths for network and parsing failures.

## Tests to request

- Route switch cleanup test.
- Watcher invalidation test for cancelled async calls.
- Store action failure rollback test.
