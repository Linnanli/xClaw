# Language Priority Mapping Rules (Triggered Mandatory)

Apply these rules only when trigger conditions are met.

## Trigger conditions

Priority mapping is **mandatory** when any one condition is true:

1. Total changed lines >= 300
2. Two or more programming languages are changed
3. Any changed path or symbol indicates high-risk runtime boundaries:
   - auth, token, permission, role, tenant
   - db, migration, transaction, repository
   - state, engine, ipc, thread, event, queue, scheduler

If none of the above is true, reviewers may use call-chain-first order.

## Required review order

1. Runtime-critical languages first:
   - Rust, Go, Java, C/C++, Python
2. UI and integration languages second:
   - TypeScript, JavaScript, Vue
3. Styles, docs, and non-runtime files last.

## Tie-breakers inside the same tier

Review files in this order:

1. Security and identity boundaries (`auth`, `token`, `permission`, `role`, `tenant`)
2. Data integrity boundaries (`db`, `migration`, `transaction`, `repository`)
3. Runtime state and orchestration (`state`, `engine`, `ipc`, `thread`, `event`, `queue`, `scheduler`)
4. UI rendering and presentation.

## Output requirement

Include a one-line note in review summary:

- `Review order applied: <tier-1 files> -> <tier-2 files> -> <tier-3 files>`

Also include whether trigger fired:

- `Priority mapping triggered: yes|no (reason)`

If order cannot be applied, explain why in the summary.
