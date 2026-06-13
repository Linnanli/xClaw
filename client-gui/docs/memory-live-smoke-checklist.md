# Memory Live Smoke Checklist

Use this checklist for manual live validation that complements the deterministic
memory smoke harness.

## Cross-Workspace Recall

- Start one session in workspace A and record a specific project fact.
- Start another session in workspace B and ask for the workspace A fact.
- Confirm the answer can recall the fact without treating workspace B as the source.

## Source Provenance

- Ask where the recalled fact came from.
- Confirm the response includes the original workspace/source context.
- Confirm unrelated workspace memories are not presented as the source.

## Non-Interactive Flows

- Run the deterministic memory smoke harness.
- Confirm non-interactive recall paths still include memory context.
- Confirm failures are visible in logs or test output instead of silently falling back.
