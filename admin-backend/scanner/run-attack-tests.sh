#!/usr/bin/env bash
set -euo pipefail

SCANNER_URL="${SCANNER_URL:-http://localhost:8000}"
SAMPLES_DIR="${SAMPLES_DIR:-$(cd "$(dirname "$0")" && pwd)/attack-samples}"
USE_LLM="${USE_LLM:-true}"
LLM_PROVIDER="${LLM_PROVIDER:-anthropic}"
LLM_API_KEY="${LLM_API_KEY:-}"

if [[ ! -d "$SAMPLES_DIR" ]]; then
  echo "[error] samples directory not found: $SAMPLES_DIR"
  exit 1
fi

if ! command -v zip >/dev/null 2>&1; then
  echo "[error] zip command not found"
  exit 1
fi

tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

echo "[info] scanner endpoint: $SCANNER_URL/scan-upload"
echo "[info] samples directory: $SAMPLES_DIR"
echo "[info] use_llm=$USE_LLM llm_provider=$LLM_PROVIDER"

total=0
blocked=0
suspicious=0
dangerous=0
safe=0
unknown=0

for sample in "$SAMPLES_DIR"/*.md; do
  [[ -e "$sample" ]] || continue
  total=$((total + 1))

  name="$(basename "$sample")"
  work_dir="$tmp_dir/${name%.md}"
  mkdir -p "$work_dir/pkg"
  cp "$sample" "$work_dir/pkg/SKILL.md"

  zip_path="$work_dir/${name%.md}.zip"
  (
    cd "$work_dir/pkg"
    zip -qr "$zip_path" .
  )

  echo "\n=== [$total] $name ==="

  curl_args=(
    -sS
    -X POST
    "$SCANNER_URL/scan-upload"
    -F "file=@$zip_path;type=application/zip"
    -F "use_llm=$USE_LLM"
    -F "llm_provider=$LLM_PROVIDER"
  )

  if [[ -n "$LLM_API_KEY" ]]; then
    curl_args+=( -H "X-LLM-Key: $LLM_API_KEY" )
  fi

  response="$(curl "${curl_args[@]}")"

  if command -v jq >/dev/null 2>&1; then
    verdict="$(printf '%s' "$response" | jq -r '
      if .verdict then .verdict
      elif .is_safe == true then "SAFE"
      elif (.max_severity == "CRITICAL" or .max_severity == "HIGH") then "DANGEROUS"
      elif .is_safe == false then "SUSPICIOUS"
      else "UNKNOWN"
      end
    ')"
    findings="$(printf '%s' "$response" | jq -r '.findings_count // 0')"
    max_severity="$(printf '%s' "$response" | jq -r '.max_severity // "UNKNOWN"')"
    echo "verdict=$verdict findings=$findings max_severity=$max_severity"

    case "$verdict" in
      SAFE) safe=$((safe + 1)) ;;
      SUSPICIOUS) suspicious=$((suspicious + 1)) ;;
      DANGEROUS) dangerous=$((dangerous + 1)) ;;
      BLOCKED) blocked=$((blocked + 1)) ;;
      *) unknown=$((unknown + 1)) ;;
    esac
  else
    echo "$response"
    unknown=$((unknown + 1))
  fi
done

echo "\n=== Summary ==="
echo "total=$total safe=$safe suspicious=$suspicious dangerous=$dangerous blocked=$blocked unknown=$unknown"
