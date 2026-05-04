# Service Migration — `ironclaw.*` → `dasclaw.*` (ADR-114 Ⅳ)

> Status: shipped via #109 (PR-link to be filled on merge). Source ADR:
> [adr-114 §2.2 类 B-Ⅳ](plans/architecture-refactor/adr-114-dasclaw-rebrand.md).

This document explains how the OS service unit names migrate from the
pre-rebrand `ironclaw.*` family to the new `dasclaw.*` family without
leaving orphan launchd plists or systemd units on user machines.

## Name table

| Platform | Old | New |
|---|---|---|
| macOS launchd label | `com.ironclaw.daemon` | `com.dasclaw.daemon` |
| macOS launchd plist | `~/Library/LaunchAgents/com.ironclaw.daemon.plist` | `~/Library/LaunchAgents/com.dasclaw.daemon.plist` |
| Linux systemd unit name | `ironclaw.service` | `dasclaw.service` |
| Linux systemd unit path | `~/.config/systemd/user/ironclaw.service` | `~/.config/systemd/user/dasclaw.service` |

## What `service install` does now

```sh
ironclaw service install
```

Always installs the **new** `dasclaw.*` family. It does not touch any
legacy `ironclaw.*` file that may still be on disk.

## Migrating an existing install

```sh
ironclaw service migrate
```

The migrate subcommand performs a single-shot upgrade in this order:

1. **Detect** any legacy `com.ironclaw.daemon.plist` (macOS) or
   `ironclaw.service` (Linux). If absent, the rest is equivalent to
   `service install` followed by `service start`.
2. **Stop legacy** (best-effort, ignores "not loaded"):
   - macOS: `launchctl stop com.ironclaw.daemon` then `launchctl unload -w …`.
   - Linux: `systemctl --user stop ironclaw.service` then
     `systemctl --user disable ironclaw.service`.
3. **Remove the legacy unit file** so its auto-load metadata cannot
   resurrect it on the next login.
4. **Install** the new `dasclaw.*` unit (idempotent — safe to re-run).
5. **Stop-then-start** the new unit so a re-run from a clean state
   does not collide with an already-loaded current install.

The command is idempotent: running it twice on a fully-migrated
machine is a no-op aside from a stop/start cycle of the current unit.

## Uninstalling

```sh
ironclaw service uninstall
```

Stops and removes **both** the new and any leftover legacy unit, so a
single `uninstall` invocation guarantees no stray daemon survives.

## Doctor

```sh
ironclaw doctor
```

The service-installed check now prefers the new unit and falls back to
the legacy unit if the new one is missing. When the legacy file is
detected, the doctor message includes a hint to run
`ironclaw service migrate`.

## Non-goals (deferred)

- Binary / crate rename (`ironclaw` → `dasclaw`) — tracked separately
  by ADR-114 Ⅴ / issue #110.
- Production-deployment systemd unit at
  `desktop-client/ironclaw/deploy/ironclaw.service` is a Docker host
  unit on cloud VMs and is **not** in scope for the user-side migrate
  flow. Operators of that unit must rename it manually together with
  their deploy automation.
- `IRONCLAW_BASE_DIR` / data directory rename — handled by the
  ADR-114 Ⅱ / Ⅲ work that already shipped.
