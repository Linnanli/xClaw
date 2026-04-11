# C and C++ Review Guide

## Scope

Use for `.c`, `.h`, `.cpp`, `.cc`, `.cxx`, `.hpp` changes.

## Memory and lifetime

- Check ownership model is explicit and consistent.
- Check buffer bounds and integer overflow/underflow.
- Check use-after-free, double-free, and dangling references.
- Check RAII usage for resource cleanup in C++.

## Concurrency and ordering

- Check data races on shared state.
- Check lock ordering and deadlock risks.
- Check atomics memory-order assumptions are documented.

## API and error handling

- Check return codes and error contracts are consistent.
- Check partial initialization cleanup paths.
- Check undefined behavior risks in casts and pointer arithmetic.

## Tests to request

- Stress tests for race conditions.
- Boundary tests for buffer and numeric limits.
- Fault-injection tests for cleanup paths.
