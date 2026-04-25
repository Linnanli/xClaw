#!/usr/bin/env bash
# 把 x_claw_ 改成 dasclaw_，保留已存在的 x_claw_agent
set -euo pipefail
target="$1"
cp "$target" "${target}.bak"
perl -i -pe 's/x_claw_(?!agent\b)/dasclaw_/g' "$target"
echo "renamed $target (backup at ${target}.bak)"
echo "x_claw_agent preserved count: $(grep -c x_claw_agent "$target" || true)"
echo "remaining non-agent x_claw_ matches:"
grep -nP 'x_claw_(?!agent)' "$target" | head || echo "  (none)"
echo "dasclaw_ total: $(grep -c dasclaw_ "$target")"
