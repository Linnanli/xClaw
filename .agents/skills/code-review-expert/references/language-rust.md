# Rust Review Guide

## Scope

Use for `.rs` changes, especially async and concurrent code.

## Ownership and lifetimes

- Check unnecessary `clone()` calls on hot paths.
- Check borrow scopes are minimal and clear.
- Check `Arc<Mutex<T>>` usage is justified and bounded.

## Async and concurrency

- Check no blocking I/O inside async context.
- Check lock guards are not held across `.await` unless intended.
- Check spawned tasks lifecycle and cancellation strategy.
- Check race-prone read-modify-write sequences on shared state.
- Check behavior under retry and duplicate request scenarios.

## Error and safety

- Avoid `unwrap()` and `expect()` in production paths.
- Check error mapping preserves actionable context without leaking secrets.
- Check `unsafe` blocks include clear safety invariants.

## Behavior regression focus

- Thread/task switch should clear per-request transient state.
- Event stream handlers should be idempotent and order-tolerant.
- State persistence and in-memory state updates should be atomic or rollback-safe.

## Tests to request

- Concurrency test (parallel requests/tasks).
- Cancellation and shutdown test.
- Failure path rollback and state consistency test.
