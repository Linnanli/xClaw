#!/bin/bash
# Admin Backend 独立启动脚本
#
# 只启动 Admin Backend（后端 API + 前端管理界面 + PostgreSQL）。
# 不启动 Desktop Client，适合只开发 Admin Backend 时使用。
#
# 用法:
#   ./scripts/start-admin.sh
#
# 前置条件:
#   - Docker 已启动（用于 PostgreSQL 容器）
#
# 如果同时需要 Desktop Client，请使用:
#   ./scripts/start-all.sh

exec "$(dirname "${BASH_SOURCE[0]}")/../admin-backend/scripts/start-admin.sh" "$@"
