# Skill Scanner Local Setup

This directory contains scanner container assets for Task 1.2b.

## Files

- Docker image definition: `scanner/Dockerfile`
- Regex append rules: `scanner/rules/signatures-append.yaml`
- YARA rules: `scanner/rules/yara/skillhub_vetter.yara`

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

- `scanner/rules/signatures-append.yaml` -> `/app/vetter-rules/signatures-append.yaml`
- `scanner/rules/yara/skillhub_vetter.yara` -> `.../skill_scanner/data/yara_rules/skillhub_vetter.yara`

## Troubleshooting

- If image pull fails with `network is unreachable`, Docker cannot access Docker Hub.
- Ensure Docker Desktop can reach `registry-1.docker.io:443`.
- Retry after network/proxy setup, then rerun health check.
