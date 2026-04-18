#!/usr/bin/env bash
# Generate a Parity Report in JSON format from parity harness test results.
#
# Usage:
#   ./scripts/parity-report.sh              # stdout JSON
#   ./scripts/parity-report.sh -o report.json  # write to file
#
# Exit codes: 0 = all passed, 1 = some failed, 2 = build error

set -euo pipefail

OUTPUT_FILE=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        -o|--output) OUTPUT_FILE="$2"; shift 2 ;;
        *) echo "Usage: $0 [-o output.json]" >&2; exit 2 ;;
    esac
done

IRONCLAW_DIR="$(cd "$(dirname "$0")/../desktop-client/ironclaw" && pwd)"

# Run tests and capture output
TEST_OUTPUT=$(cd "$IRONCLAW_DIR" && cargo test --test parity_harness 2>&1) || true

# Parse results
TOTAL=$(echo "$TEST_OUTPUT" | grep -cE '^\s*test (ps_|sp_)' || true)
PASSED=$(echo "$TEST_OUTPUT" | grep -cE '^\s*test (ps_|sp_).*\.\.\. ok$' || true)
FAILED=$(echo "$TEST_OUTPUT" | grep -cE '^\s*test (ps_|sp_).*\.\.\. FAILED$' || true)
IGNORED=$(echo "$TEST_OUTPUT" | grep -cE '^\s*test (ps_|sp_).*\.\.\. ignored' || true)

# Count by group
FP_TOTAL=$(echo "$TEST_OUTPUT" | grep -cE '^\s*test ps_' || true)
FP_PASSED=$(echo "$TEST_OUTPUT" | grep -cE '^\s*test ps_.*\.\.\. ok$' || true)
SP_TOTAL=$(echo "$TEST_OUTPUT" | grep -cE '^\s*test sp_' || true)
SP_PASSED=$(echo "$TEST_OUTPUT" | grep -cE '^\s*test sp_.*\.\.\. ok$' || true)

# Calculate score
if [ "$TOTAL" -gt 0 ]; then
    PARITY_SCORE=$(awk "BEGIN {printf \"%.1f\", ($PASSED / $TOTAL) * 100}")
else
    PARITY_SCORE="0.0"
fi

# Collect failed test names
FAILED_TESTS=$(echo "$TEST_OUTPUT" | grep -E '^\s*test (ps_|sp_).*\.\.\. FAILED$' | awk '{print $2}' | jq -R -s 'split("\n") | map(select(length > 0))') || FAILED_TESTS="[]"
IGNORED_TESTS=$(echo "$TEST_OUTPUT" | grep -E '^\s*test (ps_|sp_).*\.\.\. ignored' | awk '{print $2}' | jq -R -s 'split("\n") | map(select(length > 0))') || IGNORED_TESTS="[]"

# Extract timing
DURATION=$(echo "$TEST_OUTPUT" | grep -oE 'finished in [0-9.]+s' | grep -oE '[0-9.]+' | head -1) || DURATION="0"

TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

REPORT=$(jq -n \
  --arg ts "$TIMESTAMP" \
  --argjson total "${TOTAL:-0}" \
  --argjson passed "${PASSED:-0}" \
  --argjson failed "${FAILED:-0}" \
  --argjson ignored "${IGNORED:-0}" \
  --arg score "${PARITY_SCORE}%" \
  --argjson fp_total "${FP_TOTAL:-0}" \
  --argjson fp_passed "${FP_PASSED:-0}" \
  --argjson sp_total "${SP_TOTAL:-0}" \
  --argjson sp_passed "${SP_PASSED:-0}" \
  --argjson dur "${DURATION:-0}" \
  --argjson ft "$FAILED_TESTS" \
  --argjson it "$IGNORED_TESTS" \
  '{
    timestamp: $ts,
    total_scenarios: $total,
    passed: $passed,
    failed: $failed,
    ignored: $ignored,
    parity_score: $score,
    functional_parity: { total: $fp_total, passed: $fp_passed },
    security_parity: { total: $sp_total, passed: $sp_passed },
    cache_hit_rate: "N/A (requires live LLM)",
    avg_token_efficiency: "N/A (requires live LLM)",
    duration_secs: $dur,
    failed_tests: $ft,
    ignored_tests: $it
  }')

if [ -n "$OUTPUT_FILE" ]; then
    echo "$REPORT" > "$OUTPUT_FILE"
    echo "Report written to $OUTPUT_FILE" >&2
else
    echo "$REPORT"
fi

# Exit code based on failures
if [ "$FAILED" -gt 0 ]; then
    exit 1
fi
exit 0
