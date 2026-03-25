#!/usr/bin/env bash
# Architecture boundary checks for x-claw (adapted from ironclaw).
# Scans admin-backend/src/, desktop-client/src/, and crates/ for violations.
#
# Run as: bash scripts/check-boundaries.sh
# Returns non-zero if hard violations are found.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

violations=0

echo "=== Architecture Boundary Checks (x-claw) ==="
echo

# --------------------------------------------------------------------------
# Check 1: Direct database driver usage outside the db layer (admin-backend)
# --------------------------------------------------------------------------
# In admin-backend, raw SQL driver types (tokio_postgres::, sqlx::, libsql::)
# should only appear in db.rs and migrations-related code.
# Handlers should go through the db abstraction.
# --------------------------------------------------------------------------

echo "--- Check 1: Direct database driver usage outside db layer (admin-backend) ---"

if [ -d "admin-backend/src" ]; then
    results=$(grep -rn 'tokio_postgres::\|sqlx::\|libsql::' admin-backend/src/ \
        --include='*.rs' \
        | grep -v 'admin-backend/src/db.rs' \
        | grep -v 'admin-backend/src/main.rs' \
        | grep -v '^\s*//' \
        | grep -v '//.*tokio_postgres\|//.*sqlx\|//.*libsql' \
        || true)

    if [ -n "$results" ]; then
        echo "WARNING: Direct database driver usage found outside db layer:"
        echo "$results"
        count=$(echo "$results" | wc -l | tr -d ' ')
        echo "($count occurrence(s) — handlers should use the db abstraction)"
    else
        echo "OK"
    fi
else
    echo "SKIP: admin-backend/src not found"
fi
echo

# --------------------------------------------------------------------------
# Check 2: .unwrap() / .expect() in production code
# --------------------------------------------------------------------------

echo "--- Check 2: .unwrap() / .expect() / assert!() in production code ---"

total_unwrap=0
for src_dir in admin-backend/src desktop-client/src crates/*/src; do
    [ -d "$src_dir" ] || continue

    raw_results=$(grep -rnE '\.(unwrap|expect)\(|[^_]assert(_eq|_ne)?!' "$src_dir" \
        --include='*.rs' \
        | grep -Ev 'debug_assert|// safety:|#\[cfg\(test\)\]|#\[test\]|mod tests|fn test_' \
        || true)

    if [ -n "$raw_results" ]; then
        count=$(echo "$raw_results" | wc -l | tr -d ' ')
        total_unwrap=$((total_unwrap + count))
        echo "  $src_dir: ~$count occurrence(s)"
    fi
done

if [ "$total_unwrap" -gt 0 ]; then
    echo "WARNING: ~$total_unwrap .unwrap()/.expect()/assert!() calls found in production code."
    echo "(Gradual cleanup — many may be in #[cfg(test)] modules which is acceptable.)"
else
    echo "OK"
fi
echo

# --------------------------------------------------------------------------
# Check 3: Direct env var reads outside config/bootstrap layers
# --------------------------------------------------------------------------

echo "--- Check 3: Direct env var reads outside config layer ---"

for src_dir in admin-backend/src desktop-client/src; do
    [ -d "$src_dir" ] || continue

    results=$(grep -rn 'std::env::var\|env::var(' "$src_dir" \
        --include='*.rs' \
        | grep -v 'main.rs' \
        | grep -v '#\[cfg(test)\]' \
        | grep -v '#\[test\]' \
        | grep -v 'mod tests' \
        | grep -v 'fn test_' \
        | grep -v '//.*env::var' \
        || true)

    if [ -n "$results" ]; then
        count=$(echo "$results" | wc -l | tr -d ' ')
        echo "  $src_dir: $count direct env var read(s):"
        echo "$results" | sed 's/^/    /'
    fi
done
echo "OK (review any listed occurrences — config should come through structured config)"
echo

# --------------------------------------------------------------------------
# Check 4: Desktop Client should not directly import admin-backend types
# --------------------------------------------------------------------------

echo "--- Check 4: Desktop Client / Admin Backend isolation ---"

if [ -d "desktop-client/src" ]; then
    results=$(grep -rn 'admin_backend::' desktop-client/src/ \
        --include='*.rs' \
        | grep -v '^\s*//' \
        || true)

    if [ -n "$results" ]; then
        echo "VIOLATION: Desktop Client directly imports admin-backend types:"
        echo "$results"
        violations=$((violations + 1))
    else
        echo "OK"
    fi
else
    echo "SKIP: desktop-client/src not found"
fi
echo

# --------------------------------------------------------------------------
# Check 5: Shared crates should not depend on application-layer code
# --------------------------------------------------------------------------

echo "--- Check 5: Shared crate independence ---"

for crate_src in crates/*/src; do
    [ -d "$crate_src" ] || continue
    crate_name=$(basename "$(dirname "$crate_src")")

    results=$(grep -rn 'admin_backend::\|desktop_client::' "$crate_src" \
        --include='*.rs' \
        | grep -v '^\s*//' \
        || true)

    if [ -n "$results" ]; then
        echo "VIOLATION: crate '$crate_name' imports application-layer code:"
        echo "$results"
        violations=$((violations + 1))
    fi
done
echo "OK"
echo

# --------------------------------------------------------------------------
# Summary
# --------------------------------------------------------------------------

echo "=== Summary ==="
if [ "$violations" -gt 0 ]; then
    echo "FAILED: $violations hard violation(s) found"
    exit 1
else
    echo "PASSED: No hard violations found (review warnings above)"
    exit 0
fi
