# Java Review Guide

## Scope

Use for `.java` and Spring-related backend changes.

## Transaction and consistency

- Check transactional boundaries for multi-step writes.
- Check isolation assumptions for concurrent updates.
- Check optimistic/pessimistic locking where needed.

## Nullability and exceptions

- Check null contracts and optional usage consistency.
- Check exception translation does not hide root causes.
- Check user-facing errors avoid leaking internals.

## Concurrency and threading

- Check shared mutable state in singleton beans.
- Check executor usage, queue bounds, and shutdown behavior.
- Check interrupt handling and timeout propagation.

## API behavior

- Check validation annotations and server-side enforcement.
- Check authz checks on read and write endpoints.
- Check idempotency for retryable endpoints.

## Tests to request

- Transaction rollback tests.
- Concurrent update race tests.
- Endpoint authz and error-path tests.
