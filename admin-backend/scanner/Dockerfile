FROM python:3.11-slim

WORKDIR /app

ARG SKILL_SCANNER_VERSION=2.0.9

RUN apt-get update && \
  apt-get install -y --no-install-recommends build-essential libffi-dev ca-certificates wget && \
  rm -rf /var/lib/apt/lists/* && \
  pip install --no-cache-dir "cisco-ai-skill-scanner==${SKILL_SCANNER_VERSION}"

RUN python - <<'PY'
import pathlib
import re

import skill_scanner

pkg_dir = pathlib.Path(skill_scanner.__file__).resolve().parent

# Layer custom rules on top of official core packs.
custom_signature_link = pkg_dir / "data" / "packs" / "core" / "signatures" / "xclaw_custom.yaml"
custom_yara_link = pkg_dir / "data" / "packs" / "core" / "yara" / "xclaw_custom.yara"

for link_path, target_path in (
  (custom_signature_link, pathlib.Path("/app/custom-rules/signatures/xclaw_custom.yaml")),
  (custom_yara_link, pathlib.Path("/app/custom-rules/yara/xclaw_custom.yara")),
):
  link_path.parent.mkdir(parents=True, exist_ok=True)
  link_path.unlink(missing_ok=True)
  link_path.symlink_to(target_path)

# Add per-request LLM options for /scan and /scan-upload via headers.
router_file = pkg_dir / "api" / "router.py"
router_source = router_file.read_text(encoding="utf-8")

def replace_once(source: str, old: str, new: str) -> str:
  if old not in source:
    raise RuntimeError(f"patch marker not found: {old[:80]!r}")
  return source.replace(old, new, 1)

def replace_regex_once(source: str, pattern: str, repl: str) -> str:
  replaced, count = re.subn(pattern, repl, source, count=1, flags=re.MULTILINE)
  if count == 0:
    raise RuntimeError(f"patch regex not found: {pattern[:80]!r}")
  return replaced

if 'alias="X-LLM-Key"' not in router_source:
  router_source = replace_regex_once(
    router_source,
    r"(\s+llm_provider:\s*str \| None = \"anthropic\",\n)(\s+use_virustotal:\s*bool = False,)",
    r"\1    llm_api_key: str | None = None,\n\2",
  )
  router_source = replace_regex_once(
    router_source,
    r"(\s+llm_provider=llm_provider,\n)(\s+use_virustotal=use_virustotal,)",
    r"\1          llm_api_key=llm_api_key,\n\2",
  )
  router_source = replace_once(
    router_source,
    "async def scan_skill(\n    request: ScanRequest,\n    vt_api_key: str | None = Header(None, alias=\"X-VirusTotal-Key\"),",
    "async def scan_skill(\n    request: ScanRequest,\n    llm_api_key: str | None = Header(None, alias=\"X-LLM-Key\"),\n    vt_api_key: str | None = Header(None, alias=\"X-VirusTotal-Key\"),",
  )
  router_source = replace_once(
    router_source,
    "            llm_provider=request.llm_provider,\n            use_virustotal=request.use_virustotal,",
    "            llm_provider=request.llm_provider,\n            llm_api_key=llm_api_key,\n            use_virustotal=request.use_virustotal,",
  )
  router_source = replace_once(
    router_source,
    "    llm_provider: str = Form(\"anthropic\", description=\"LLM provider\"),",
    "    llm_provider: str = Form(\"anthropic\", description=\"LLM provider\"),\n    llm_api_key: str | None = Header(None, alias=\"X-LLM-Key\"),",
  )
  router_source = replace_once(
    router_source,
    "        return await scan_skill(request, vt_api_key=vt_api_key, aidefense_api_key=aidefense_api_key)",
    "        return await scan_skill(\n            request,\n            llm_api_key=llm_api_key,\n            vt_api_key=vt_api_key,\n            aidefense_api_key=aidefense_api_key,\n        )",
  )

if 'alias="X-LLM-Model"' not in router_source:
  router_source = replace_regex_once(
    router_source,
    r"(\s+llm_api_key:\s*str \| None = None,\n)(\s+use_virustotal:\s*bool = False,)",
    r"\1    llm_model: str | None = None,\n    llm_base_url: str | None = None,\n    llm_api_version: str | None = None,\n\2",
  )
  router_source = replace_regex_once(
    router_source,
    r"(\s+llm_api_key=llm_api_key,\n)(\s+use_virustotal=use_virustotal,)",
    r"\1        llm_model=llm_model,\n        llm_base_url=llm_base_url,\n        llm_api_version=llm_api_version,\n\2",
  )
  router_source = replace_once(
    router_source,
    "async def scan_skill(\n    request: ScanRequest,\n    llm_api_key: str | None = Header(None, alias=\"X-LLM-Key\"),\n    vt_api_key: str | None = Header(None, alias=\"X-VirusTotal-Key\"),",
    "async def scan_skill(\n    request: ScanRequest,\n    llm_api_key: str | None = Header(None, alias=\"X-LLM-Key\"),\n    llm_model: str | None = Header(None, alias=\"X-LLM-Model\"),\n    llm_base_url: str | None = Header(None, alias=\"X-LLM-Base-URL\"),\n    llm_api_version: str | None = Header(None, alias=\"X-LLM-API-Version\"),\n    vt_api_key: str | None = Header(None, alias=\"X-VirusTotal-Key\"),",
  )
  router_source = replace_once(
    router_source,
    "            llm_provider=request.llm_provider,\n            llm_api_key=llm_api_key,\n            use_virustotal=request.use_virustotal,",
    "            llm_provider=request.llm_provider,\n            llm_api_key=llm_api_key,\n            llm_model=llm_model,\n            llm_base_url=llm_base_url,\n            llm_api_version=llm_api_version,\n            use_virustotal=request.use_virustotal,",
  )
  router_source = replace_once(
    router_source,
    "    llm_provider: str = Form(\"anthropic\", description=\"LLM provider\"),\n    llm_api_key: str | None = Header(None, alias=\"X-LLM-Key\"),",
    "    llm_provider: str = Form(\"anthropic\", description=\"LLM provider\"),\n    llm_api_key: str | None = Header(None, alias=\"X-LLM-Key\"),\n    llm_model: str | None = Header(None, alias=\"X-LLM-Model\"),\n    llm_base_url: str | None = Header(None, alias=\"X-LLM-Base-URL\"),\n    llm_api_version: str | None = Header(None, alias=\"X-LLM-API-Version\"),",
  )
  router_source = replace_once(
    router_source,
    "        return await scan_skill(\n            request,\n            llm_api_key=llm_api_key,\n            vt_api_key=vt_api_key,\n            aidefense_api_key=aidefense_api_key,\n        )",
    "        return await scan_skill(\n            request,\n            llm_api_key=llm_api_key,\n            llm_model=llm_model,\n            llm_base_url=llm_base_url,\n            llm_api_version=llm_api_version,\n            vt_api_key=vt_api_key,\n            aidefense_api_key=aidefense_api_key,\n        )",
  )

router_file.write_text(router_source, encoding="utf-8")
PY

RUN groupadd -r app && \
  useradd -r -g app app && \
  mkdir -p /app/custom-rules/signatures && \
  mkdir -p /app/custom-rules/yara && \
  mkdir -p /tmp/skillhub-scans && \
  chown -R app:app /tmp/skillhub-scans /app/custom-rules

USER app

EXPOSE 8000

HEALTHCHECK --interval=10s --timeout=3s \
  CMD wget -qO- http://127.0.0.1:8000/health || exit 1

CMD ["skill-scanner-api", "--host", "0.0.0.0", "--port", "8000"]
