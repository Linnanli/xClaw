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

exec "$(dirname "${BASH_SOURCE[0]}")/../desktop-client/scripts/start-dev.sh" "$@"
