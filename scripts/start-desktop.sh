#!/bin/bash
# Desktop Client 独立启动脚本
#
# 只启动 Desktop Client（前端 dev server + Tauri + 嵌入式 IronClaw 引擎）。
# 不启动 Admin Backend，适合只开发 Desktop Client 时使用。
#
# 用法:
#   ./scripts/start-desktop.sh
#
# 前置条件:
#   - LLM 配置（desktop-client/.env 或 admin_config.json 或环境变量）
#
# 如果同时需要 Admin Backend（DLP 规则同步、策略管理等），请使用:
#   ./scripts/start-all.sh

if [ -z "${SECRETS_MASTER_KEY:-}" ] && [ "${ALLOW_INSECURE_DEV_SECRETS:-0}" = "1" ]; then
	export SECRETS_MASTER_KEY="0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
	echo "[WARN] ALLOW_INSECURE_DEV_SECRETS=1，已注入开发固定密钥（仅本机调试）"
elif [ -z "${SECRETS_MASTER_KEY:-}" ]; then
	echo "[WARN] SECRETS_MASTER_KEY 未设置；如需绕过 Keychain 弹窗，请显式设置 SECRETS_MASTER_KEY 或 ALLOW_INSECURE_DEV_SECRETS=1"
fi

exec "$(dirname "${BASH_SOURCE[0]}")/../desktop-client/scripts/start-dev.sh" "$@"
