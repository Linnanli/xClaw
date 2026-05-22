#!/usr/bin/env python3
"""F4.3 import rewriter — replaces `crate::secrets::*` shim imports with
direct references to `dasclaw_runtime::secrets::*`.

Usage:
    python3 scripts/f43_rewrite_secrets_imports.py <file>...

Each input file is rewritten in place.

The host-specific entries below stay rooted at `crate::secrets::*` because
they depend on ironclaw-only types (DatabaseHandles, IRONCLAW_KEYCHAIN
brand, etc.):

    crate::secrets::keychain::*
    crate::secrets::create_secrets_store
    crate::secrets::resolve_master_key
    crate::secrets::crypto_from_hex

Every other reference to `crate::secrets::X` is rewritten to
`dasclaw_runtime::secrets::X`, with these special cases collapsed onto
the runtime crate's actual public API:

    crate::secrets::store::in_memory::InMemorySecretsStore
        -> dasclaw_runtime::secrets::InMemorySecretsStore
    crate::secrets::store::SecretsStore
        -> dasclaw_runtime::secrets::SecretsStore
    crate::secrets::store::<anything>
        -> dasclaw_runtime::secrets::<anything>
"""

import re
import sys
from pathlib import Path

HOST_LOCAL = (
    "keychain",
    "create_secrets_store",
    "resolve_master_key",
    "crypto_from_hex",
)

# Both ironclaw test code (uses `ironclaw::secrets::*`) and ironclaw library
# code (uses `crate::secrets::*`) need rewriting.
PREFIX = r"(?:crate|ironclaw)::secrets"

# Order matters: more specific paths first.
SPECIFIC = [
    (
        re.compile(PREFIX + r"::store::in_memory::InMemorySecretsStore"),
        "dasclaw_runtime::secrets::InMemorySecretsStore",
    ),
    (
        re.compile(PREFIX + r"::store::SecretsStore"),
        "dasclaw_runtime::secrets::SecretsStore",
    ),
    (
        re.compile(PREFIX + r"::store::"),
        "dasclaw_runtime::secrets::",
    ),
    (
        re.compile(r"use\s+(?:crate|ironclaw)::secrets::\{"),
        "use dasclaw_runtime::secrets::{",
    ),
]

# Match (crate|ironclaw)::secrets::<segment> where <segment> is the first
# identifier following the prefix. Capture it so we can decide whether to
# rewrite.
GENERIC = re.compile(PREFIX + r"::([A-Za-z_][A-Za-z0-9_]*)")


def rewrite_text(text: str) -> tuple[str, int]:
    count = 0
    for pat, repl in SPECIFIC:
        text, n = pat.subn(repl, text)
        count += n

    def sub(match: re.Match) -> str:
        nonlocal count
        seg = match.group(1)
        if seg in HOST_LOCAL:
            return match.group(0)  # leave host-specific paths alone
        count += 1
        full = match.group(0)
        # Replace the (crate|ironclaw)::secrets:: prefix with the runtime path.
        return re.sub(r"^(?:crate|ironclaw)::secrets", "dasclaw_runtime::secrets", full)

    text = GENERIC.sub(sub, text)
    return text, count


def process_file(path: Path) -> int:
    text = path.read_text(encoding="utf-8")
    new_text, count = rewrite_text(text)
    if count:
        path.write_text(new_text, encoding="utf-8")
    return count


def main() -> None:
    total = 0
    for arg in sys.argv[1:]:
        n = process_file(Path(arg))
        if n:
            print(f"{arg}: {n}")
        total += n
    print(f"total: {total}")


if __name__ == "__main__":
    main()
