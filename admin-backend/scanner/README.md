# Skill Scanner Local Setup

This directory contains scanner container assets for Task 1.2b.

## Files

- Docker image definition: `scanner/Dockerfile`
- Custom signature rules: `scanner/rules/signatures-append.yaml`
- Custom YARA rules: `scanner/rules/yara/skillhub_vetter.yara`

## Start scanner service

From `admin-backend/`:

```bash
docker compose up -d skill-scanner
```

## Health check

```bash
curl -fsS http://127.0.0.1:8000/health
```

Expected result: HTTP 200.

## Rule mounting

The compose service mounts:

- `scanner/rules/signatures-append.yaml` -> `/app/custom-rules/signatures/xclaw_custom.yaml`
- `scanner/rules/yara/skillhub_vetter.yara` -> `/app/custom-rules/yara/xclaw_custom.yara`

The image links these files into the official Core pack directories used by
`cisco-ai-skill-scanner` 2.0.9:

- `<site-packages>/skill_scanner/data/packs/core/signatures/xclaw_custom.yaml`
- `<site-packages>/skill_scanner/data/packs/core/yara/xclaw_custom.yara`

This keeps official Core rules enabled while adding local custom rules.

## LLM scan settings

`/scan-upload` is called with `use_llm` and `llm_provider` form fields.

- Global key (container-level): `SKILL_SCANNER_LLM_API_KEY`
- Optional per-upload key (request-level): `X-LLM-Key` header

Per-upload key support is patched in the scanner image build for this project.

## Attack sample validation

Sample payloads are provided in `scanner/attack-samples/`.

Run all samples against `/scan-upload`:

```bash
cd admin-backend/scanner
./run-attack-tests.sh
```

Optional environment overrides:

```bash
USE_LLM=true \
LLM_PROVIDER=anthropic \
LLM_API_KEY=<your-key> \
SCANNER_URL=http://127.0.0.1:8000 \
./run-attack-tests.sh
```

The script prints per-sample verdict/findings and a final summary.

## Troubleshooting

- If image pull fails with `network is unreachable`, Docker cannot access Docker Hub.
- Ensure Docker Desktop can reach `registry-1.docker.io:443`.
- Retry after network/proxy setup, then rerun health check.
