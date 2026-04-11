# Go Review Guide

## Scope

Use for `.go` changes.

## Concurrency and context

- Check goroutines have bounded lifecycle (no leaks).
- Check all outbound calls accept and propagate `context.Context`.
- Check channel close/send ownership rules and deadlock risks.
- Check shared state synchronization (`sync.Mutex`, `atomic`, channel patterns).

## Error handling

- Check returned errors are wrapped with context.
- Check deferred cleanup and rollback on partial failure.
- Check retry loops have backoff and stop conditions.

## API and data integrity

- Check nil map/slice behavior at boundaries.
- Check request validation and defaulting logic.
- Check transactions and multi-step writes are atomic.

## Tests to request

- Goroutine leak and cancellation tests.
- Parallel request consistency tests.
- Error rollback and retry behavior tests.
