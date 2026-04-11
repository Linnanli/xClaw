# Python Review Guide

## Scope

Use for `.py` changes in services, jobs, and scripts.

## Correctness and safety

- Check mutable default arguments and shared mutable globals.
- Check exception handling does not swallow critical failures.
- Check input validation and serialization boundaries.
- Check resource cleanup (`with` context, file and socket handles).

## Async and concurrency

- Check blocking calls inside async functions.
- Check cancellation behavior in long-running tasks.
- Check race conditions on shared caches and class-level state.
- Check retry logic for idempotency and duplicate effects.

## Data and performance

- Check N+1 query patterns and missing batch operations.
- Check unbounded loops/collections in request paths.
- Check expensive operations in high-frequency handlers.

## Tests to request

- Exception-path and timeout tests.
- Concurrent request/task consistency tests.
- State cleanup tests after failures.
