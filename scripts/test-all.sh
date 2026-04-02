#!/usr/bin/env bash
# 全量测试脚本 — Desktop Client + Admin Backend
#
# 覆盖维度：
#   Rust 单元/集成/契约/冒烟/回归/属性测试
#   前端 Vitest 单元测试（含覆盖率）
#   前端 Cypress E2E 测试（可选，需要运行中的服务）
#
# 用法：
#   ./scripts/test-all.sh              # 全量（跳过 E2E）
#   ./scripts/test-all.sh --e2e        # 包含 E2E（需要服务已启动）
#   ./scripts/test-all.sh --only rust  # 只跑 Rust 测试
#   ./scripts/test-all.sh --only fe    # 只跑前端测试
#   ./scripts/test-all.sh --only desktop
#   ./scripts/test-all.sh --only admin
#
# 环境变量：
#   SKIP_BUILD=1    跳过 cargo build 编译验证（加速调试）
#   DB_HOST         数据库地址（默认 localhost，集成测试用）
#   DB_USER         数据库用户（默认 postgres）
#   DB_PASSWORD     数据库密码（默认 postgres）
#   DB_NAME         数据库名（默认 ironclaw）

set -euo pipefail

# ── 颜色输出 ──────────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m'

pass() { echo -e "${GREEN}✓${NC} $*"; }
fail() { echo -e "${RED}✗${NC} $*"; }
info() { echo -e "${BLUE}▶${NC} $*"; }
warn() { echo -e "${YELLOW}⚠${NC}  $*"; }
section() { echo -e "\n${BOLD}══ $* ══${NC}"; }

# ── 参数解析 ──────────────────────────────────────────────────────────────────
RUN_E2E=0
ONLY=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --e2e)       RUN_E2E=1; shift ;;
        --only)      ONLY="$2"; shift 2 ;;
        *) echo "未知参数: $1"; exit 1 ;;
    esac
done

# ── 路径 ──────────────────────────────────────────────────────────────────────
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DESKTOP_RUST="$ROOT/desktop-client"
DESKTOP_UI="$ROOT/desktop-client/src-ui"
ADMIN_RUST="$ROOT/admin-backend"
ADMIN_UI="$ROOT/admin-backend/ui"

cd "$ROOT"

# ── 结果追踪 ──────────────────────────────────────────────────────────────────
FAILED=()

run_step() {
    local label="$1"; shift
    info "$label"
    if "$@"; then
        pass "$label"
    else
        fail "$label"
        FAILED+=("$label")
    fi
}

# ══════════════════════════════════════════════════════════════════════════════
# RUST — Desktop Client
# ══════════════════════════════════════════════════════════════════════════════
run_desktop_rust() {
    section "Desktop Client — Rust"

    # 1. 编译验证（0 错误 0 警告）
    if [[ "${SKIP_BUILD:-0}" != "1" ]]; then
        run_step "desktop-client: cargo build" \
            cargo build -p desktop-client 2>&1
    fi

    # 2. 单元测试（lib 内联测试）
    run_step "desktop-client: 单元测试 (lib)" \
        cargo test -p desktop-client --lib -- --test-threads=4

    # 3. 启动时序测试（Tauri 状态管理专项）
    run_step "desktop-client: 启动时序测试" \
        cargo test -p desktop-client --lib engine_startup_tests -- --test-threads=1

    # 4. 认证集成测试
    run_step "desktop-client: auth 集成测试" \
        cargo test -p desktop-client --test auth_integration_tests

    # 5. 认证需求测试
    run_step "desktop-client: auth 需求测试" \
        cargo test -p desktop-client --test auth_requirements_tests

    # 6. 认证回归测试
    run_step "desktop-client: auth 回归测试" \
        cargo test -p desktop-client --test auth_regression_tests

    # 7. 认证可靠性测试
    run_step "desktop-client: auth 可靠性测试" \
        cargo test -p desktop-client --test auth_reliability_tests

    # 8. Token 管理器测试
    run_step "desktop-client: auth token manager 测试" \
        cargo test -p desktop-client --test auth_token_manager_tests

    # 9. 策略同步属性测试
    run_step "desktop-client: policy sync 属性测试" \
        cargo test -p desktop-client --test policy_sync_property_tests

    # 10. Tauri 命令契约测试（IPC 路由完整性）
    run_step "desktop-client: Tauri 命令契约测试" \
        cargo test -p desktop-client --test tauri_command_contract_tests
}

# ══════════════════════════════════════════════════════════════════════════════
# RUST — Admin Backend
# ══════════════════════════════════════════════════════════════════════════════
run_admin_rust() {
    section "Admin Backend — Rust"

    # 1. 编译验证
    if [[ "${SKIP_BUILD:-0}" != "1" ]]; then
        run_step "admin-backend: cargo build" \
            cargo build -p admin-backend 2>&1
    fi

    # 2. 单元测试（lib 内联）
    run_step "admin-backend: 单元测试 (lib)" \
        cargo test -p admin-backend --lib -- --test-threads=4

    # 3. 冒烟测试（编译+迁移+路由三层防护）
    run_step "admin-backend: 集成冒烟测试" \
        cargo test -p admin-backend --test integration_smoke_tests \
        -- --test-threads=1

    # 4. 认证属性测试
    run_step "admin-backend: auth 属性测试" \
        cargo test -p admin-backend --test auth_property_tests

    # 5. 审计完整性测试
    run_step "admin-backend: 审计完整性测试" \
        cargo test -p admin-backend --test audit_integrity_tests

    # 6. 审计日志导出测试
    run_step "admin-backend: 审计日志导出测试" \
        cargo test -p admin-backend --test audit_log_export_tests

    # 7. 客户端配置单元测试
    run_step "admin-backend: client config 单元测试" \
        cargo test -p admin-backend --test client_config_reports_unit_tests

    # 8. 客户端配置契约测试
    run_step "admin-backend: client config 契约测试" \
        cargo test -p admin-backend --test client_config_reports_contract_tests

    # 9. 客户端配置失败路径测试
    run_step "admin-backend: client config 失败路径测试" \
        cargo test -p admin-backend --test client_config_reports_failure_tests

    # 10. 客户端配置可靠性测试
    run_step "admin-backend: client config 可靠性测试" \
        cargo test -p admin-backend --test client_config_reports_reliability_tests

    # 11. 客户端配置安全审计测试
    run_step "admin-backend: client config 安全审计测试" \
        cargo test -p admin-backend --test client_config_reports_security_audit_tests

    # 12. 客户端操作测试
    run_step "admin-backend: client operations 测试" \
        cargo test -p admin-backend --test client_operations_tests

    # 13. 客户端配置集成测试
    run_step "admin-backend: client config 集成测试" \
        cargo test -p admin-backend --test client_config_tests

    # 14. 仪表盘单元测试
    run_step "admin-backend: dashboard 单元测试" \
        cargo test -p admin-backend --test dashboard_unit_tests

    # 15. 部门单元测试
    run_step "admin-backend: department 单元测试" \
        cargo test -p admin-backend --test department_unit_tests

    # 16. Handler 测试
    run_step "admin-backend: handlers 测试" \
        cargo test -p admin-backend --test handlers_tests

    # 17. 模型配置单元测试
    run_step "admin-backend: model config 单元测试" \
        cargo test -p admin-backend --test model_config_unit_tests

    # 18. 模型配置契约测试
    run_step "admin-backend: model config 契约测试" \
        cargo test -p admin-backend --test model_config_contract_tests

    # 19. 模型配置失败路径测试
    run_step "admin-backend: model config 失败路径测试" \
        cargo test -p admin-backend --test model_config_failure_tests

    # 20. 模型配置安全审计测试
    run_step "admin-backend: model config 安全审计测试" \
        cargo test -p admin-backend --test model_config_security_audit_tests

    # 21. 技能/插件切换测试
    run_step "admin-backend: skill/plugin toggle 测试" \
        cargo test -p admin-backend --test skill_plugin_toggle_tests

    # 22. 用户编辑测试
    run_step "admin-backend: user edit 测试" \
        cargo test -p admin-backend --test user_edit_tests
}

# ══════════════════════════════════════════════════════════════════════════════
# 前端 — Desktop Client (Vitest)
# ══════════════════════════════════════════════════════════════════════════════
run_desktop_fe() {
    section "Desktop Client — 前端 (Vitest)"

    if [[ ! -d "$DESKTOP_UI/node_modules" ]]; then
        warn "node_modules 不存在，先安装依赖..."
        run_step "desktop-client/src-ui: npm install" \
            npm install --prefix "$DESKTOP_UI"
    fi

    # 单元测试（单次运行，不进入 watch 模式）
    run_step "desktop-client/src-ui: vitest 单元测试" \
        npm run test --prefix "$DESKTOP_UI" -- --run

    # 覆盖率报告
    run_step "desktop-client/src-ui: vitest 覆盖率" \
        npm run test:coverage --prefix "$DESKTOP_UI" -- --run
}

# ══════════════════════════════════════════════════════════════════════════════
# 前端 — Admin Backend (Vitest)
# ══════════════════════════════════════════════════════════════════════════════
run_admin_fe() {
    section "Admin Backend — 前端 (Vitest)"

    if [[ ! -d "$ADMIN_UI/node_modules" ]]; then
        warn "node_modules 不存在，先安装依赖..."
        run_step "admin-backend/ui: npm install" \
            npm install --prefix "$ADMIN_UI"
    fi

    # 单元测试
    run_step "admin-backend/ui: vitest 单元测试" \
        npm run test --prefix "$ADMIN_UI" -- --run 2>/dev/null || \
        warn "admin-backend/ui 暂无 vitest 配置，跳过"
}

# ══════════════════════════════════════════════════════════════════════════════
# E2E — Cypress（可选）
# ══════════════════════════════════════════════════════════════════════════════
run_e2e() {
    section "E2E 测试 (Cypress)"

    # Desktop Client E2E
    if [[ -f "$DESKTOP_UI/cypress.config.ts" ]]; then
        run_step "desktop-client/src-ui: cypress e2e" \
            npm run e2e --prefix "$DESKTOP_UI"
    else
        warn "desktop-client/src-ui: 无 cypress.config.ts，跳过"
    fi

    # Admin Backend E2E
    if [[ -f "$ADMIN_UI/cypress.config.ts" ]]; then
        run_step "admin-backend/ui: cypress e2e" \
            npm run e2e --prefix "$ADMIN_UI"
    else
        warn "admin-backend/ui: 无 cypress.config.ts，跳过"
    fi
}

# ══════════════════════════════════════════════════════════════════════════════
# 主流程
# ══════════════════════════════════════════════════════════════════════════════
START=$(date +%s)

case "$ONLY" in
    rust)
        run_desktop_rust
        run_admin_rust
        ;;
    fe)
        run_desktop_fe
        run_admin_fe
        ;;
    desktop)
        run_desktop_rust
        run_desktop_fe
        [[ "$RUN_E2E" == "1" ]] && run_e2e
        ;;
    admin)
        run_admin_rust
        run_admin_fe
        [[ "$RUN_E2E" == "1" ]] && run_e2e
        ;;
    "")
        run_desktop_rust
        run_admin_rust
        run_desktop_fe
        run_admin_fe
        [[ "$RUN_E2E" == "1" ]] && run_e2e
        ;;
    *)
        echo "未知 --only 值: $ONLY（可选: rust / fe / desktop / admin）"
        exit 1
        ;;
esac

# ── 汇总 ──────────────────────────────────────────────────────────────────────
END=$(date +%s)
ELAPSED=$((END - START))

echo ""
echo -e "${BOLD}══ 测试结果汇总 ══${NC}"
echo "耗时: ${ELAPSED}s"

if [[ ${#FAILED[@]} -eq 0 ]]; then
    echo -e "${GREEN}${BOLD}全部通过 ✓${NC}"
    exit 0
else
    echo -e "${RED}${BOLD}失败 ${#FAILED[@]} 项：${NC}"
    for f in "${FAILED[@]}"; do
        echo -e "  ${RED}✗${NC} $f"
    done
    exit 1
fi
