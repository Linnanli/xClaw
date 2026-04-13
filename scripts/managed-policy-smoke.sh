#!/bin/bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

ADMIN_BASE_URL="${ADMIN_BASE_URL:-http://127.0.0.1:3000}"
CLIENT_ID="${CLIENT_ID:-${1:-}}"
CLIENT_TOKEN="${CLIENT_TOKEN:-${CLIENT_ID:-}}"
REQUEST_INTERVAL_SECONDS="${REQUEST_INTERVAL_SECONDS:-1}"
TMP_POLICY_FIRST=""
TMP_POLICY_SECOND=""

if ! command -v curl >/dev/null 2>&1; then
    echo "[ERROR] curl is required"
    exit 1
fi

if ! command -v jq >/dev/null 2>&1; then
    echo "[ERROR] jq is required"
    exit 1
fi

if [ -z "$CLIENT_TOKEN" ]; then
    echo "[ERROR] CLIENT_TOKEN is required"
    echo "Usage: CLIENT_TOKEN=<registered_client_id_or_token> $0 [client_id]"
    echo "Example: CLIENT_TOKEN=550e8400-e29b-41d4-a716-446655440000 $0 550e8400-e29b-41d4-a716-446655440000"
    exit 1
fi

build_url() {
    local url="${ADMIN_BASE_URL%/}/api/client-policy"
    if [ -n "$CLIENT_ID" ]; then
        url="$url?client_id=$CLIENT_ID"
    fi
    echo "$url"
}

fetch_policy() {
    local output_file="$1"
    local url
    url="$(build_url)"

    local status
    status=$(curl -sS -o "$output_file" -w "%{http_code}" \
        -H "Authorization: Bearer $CLIENT_TOKEN" \
        "$url")

    if [ "$status" -ne 200 ]; then
        echo "[ERROR] /api/client-policy returned HTTP $status"
        echo "[ERROR] Response:"
        cat "$output_file"
        return 1
    fi
}

validate_envelope() {
    local input_file="$1"

    jq -e '.algorithm == "ed25519"' "$input_file" >/dev/null
    jq -e '.key_id | type == "string" and length > 0' "$input_file" >/dev/null
    jq -e '.manifest_payload | type == "string" and length > 0' "$input_file" >/dev/null
    jq -e '.signature | type == "string" and length > 0' "$input_file" >/dev/null
}

extract_manifest_field() {
    local input_file="$1"
    local field="$2"
    jq -r --arg field "$field" '.manifest_payload | fromjson | .[$field]' "$input_file"
}

validate_manifest_shape() {
    local input_file="$1"

    jq -e '.manifest_payload | fromjson | has("policy_version")' "$input_file" >/dev/null
    jq -e '.manifest_payload | fromjson | has("issued_at")' "$input_file" >/dev/null
    jq -e '.manifest_payload | fromjson | has("expires_at")' "$input_file" >/dev/null
    jq -e '.manifest_payload | fromjson | has("managed_mode")' "$input_file" >/dev/null
    jq -e '.manifest_payload | fromjson | (.allowed_skills | type == "array")' "$input_file" >/dev/null
    jq -e '.manifest_payload | fromjson | (.allowed_extensions | type == "array")' "$input_file" >/dev/null
}

validate_monotonic_version() {
    local first_file="$1"
    local second_file="$2"

    local first_version
    local second_version
    first_version=$(extract_manifest_field "$first_file" "policy_version")
    second_version=$(extract_manifest_field "$second_file" "policy_version")

    if [ "$second_version" -le "$first_version" ]; then
        echo "[ERROR] policy_version is not monotonic: first=$first_version second=$second_version"
        return 1
    fi

    echo "[OK] policy_version monotonic: first=$first_version second=$second_version"
}

main() {
    TMP_POLICY_FIRST="$(mktemp)"
    TMP_POLICY_SECOND="$(mktemp)"
    trap 'rm -f "${TMP_POLICY_FIRST:-}" "${TMP_POLICY_SECOND:-}"' EXIT

    echo "[INFO] Fetching first policy envelope..."
    fetch_policy "$TMP_POLICY_FIRST"
    validate_envelope "$TMP_POLICY_FIRST"
    validate_manifest_shape "$TMP_POLICY_FIRST"

    local key_id
    local policy_version
    local managed_mode
    key_id=$(jq -r '.key_id' "$TMP_POLICY_FIRST")
    policy_version=$(extract_manifest_field "$TMP_POLICY_FIRST" "policy_version")
    managed_mode=$(extract_manifest_field "$TMP_POLICY_FIRST" "managed_mode")

    echo "[OK] Envelope fields are valid"
    echo "[INFO] key_id=$key_id"
    echo "[INFO] policy_version=$policy_version"
    echo "[INFO] managed_mode=$managed_mode"

    sleep "$REQUEST_INTERVAL_SECONDS"

    echo "[INFO] Fetching second policy envelope to check anti-replay monotonic version..."
    fetch_policy "$TMP_POLICY_SECOND"
    validate_envelope "$TMP_POLICY_SECOND"
    validate_manifest_shape "$TMP_POLICY_SECOND"
    validate_monotonic_version "$TMP_POLICY_FIRST" "$TMP_POLICY_SECOND"

    echo "[DONE] Managed policy smoke check passed"
    echo "[TIP] To verify desktop side behavior, run desktop-client and inspect logs for key_id/policy_version/replay warnings"
}

main "$@"
