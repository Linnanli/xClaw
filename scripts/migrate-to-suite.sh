#!/bin/bash
# 架构重构迁移脚本 - 方案B执行
# 将 Desktop Client 和 Admin Backend 迁移到新的套件架构

set -e  # 遇到错误立即退出

echo "=========================================="
echo "IronClaw Suite 架构重构 - 方案B"
echo "=========================================="
echo ""

# 颜色定义
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# 当前目录
CURRENT_DIR=$(pwd)
SUITE_DIR="${CURRENT_DIR}/ironclaw-suite-new"

echo -e "${YELLOW}步骤 1/8: 创建新仓库结构${NC}"
echo "创建目录: ${SUITE_DIR}"

# 创建目录结构
mkdir -p "${SUITE_DIR}"
mkdir -p "${SUITE_DIR}/crates"
mkdir -p "${SUITE_DIR}/docs"
mkdir -p "${SUITE_DIR}/scripts"

echo -e "${GREEN}✓ 目录结构创建完成${NC}"
echo ""

echo -e "${YELLOW}步骤 2/8: 复制纯净的 IronClaw${NC}"
# 复制 ironclaw-upstream 到新仓库
cp -r "${CURRENT_DIR}/ironclaw-upstream" "${SUITE_DIR}/ironclaw"
echo -e "${GREEN}✓ IronClaw 核心复制完成${NC}"
echo ""

echo -e "${YELLOW}步骤 3/8: 迁移共享 Crate${NC}"
# 复制共享 Crate
cp -r "${CURRENT_DIR}/crates/ironclaw_auth" "${SUITE_DIR}/crates/"
cp -r "${CURRENT_DIR}/crates/ironclaw_safety" "${SUITE_DIR}/crates/"
echo -e "${GREEN}✓ 共享 Crate 迁移完成${NC}"
echo ""

echo -e "${YELLOW}步骤 4/8: 迁移 Desktop Client${NC}"
# 复制 Desktop Client
cp -r "${CURRENT_DIR}/desktop-client" "${SUITE_DIR}/"
echo -e "${GREEN}✓ Desktop Client 迁移完成${NC}"
echo ""

echo -e "${YELLOW}步骤 5/8: 迁移 Admin Backend${NC}"
# 复制 Admin Backend
cp -r "${CURRENT_DIR}/admin-backend" "${SUITE_DIR}/"
echo -e "${GREEN}✓ Admin Backend 迁移完成${NC}"
echo ""

echo -e "${YELLOW}步骤 6/8: 复制文档和配置${NC}"
# 复制重要文档
cp "${CURRENT_DIR}/AGENTS.md" "${SUITE_DIR}/"
cp "${CURRENT_DIR}/IRONCLAW_CORE_MODIFICATIONS.md" "${SUITE_DIR}/docs/"
cp "${CURRENT_DIR}/ARCHITECTURE_REFACTOR_PLAN_B.md" "${SUITE_DIR}/docs/"
cp "${CURRENT_DIR}/.gitignore" "${SUITE_DIR}/"
echo -e "${GREEN}✓ 文档复制完成${NC}"
echo ""

echo -e "${YELLOW}步骤 7/8: 创建工作空间配置${NC}"
# 这一步将在下一个脚本中完成
echo "工作空间配置将在下一步创建"
echo -e "${GREEN}✓ 准备完成${NC}"
echo ""

echo -e "${YELLOW}步骤 8/8: 生成迁移报告${NC}"
cat > "${SUITE_DIR}/MIGRATION_REPORT.md" << 'EOF'
# 架构迁移报告

## 迁移时间
$(date)

## 迁移内容

### 1. IronClaw 核心
- 版本: v0.18.0
- 状态: 纯净版本，无修改
- 位置: ironclaw/

### 2. 共享 Crate
- ironclaw_auth: 认证模块
- ironclaw_safety: 安全和 DLP 模块
- 位置: crates/

### 3. 应用层
- Desktop Client: desktop-client/
- Admin Backend: admin-backend/

### 4. 文档
- AGENTS.md: 开发规则
- IRONCLAW_CORE_MODIFICATIONS.md: 核心修改记录
- ARCHITECTURE_REFACTOR_PLAN_B.md: 重构计划

## 下一步

1. 创建工作空间 Cargo.toml
2. 更新依赖路径
3. 创建 ironclaw_crypto Crate
4. 运行测试验证
5. 初始化 Git 仓库

详见 ARCHITECTURE_REFACTOR_PLAN_B.md
EOF

echo -e "${GREEN}✓ 迁移报告生成完成${NC}"
echo ""

echo "=========================================="
echo -e "${GREEN}迁移完成！${NC}"
echo "=========================================="
echo ""
echo "新仓库位置: ${SUITE_DIR}"
echo ""
echo "下一步操作:"
echo "1. cd ${SUITE_DIR}"
echo "2. 运行 scripts/setup-workspace.sh 创建工作空间配置"
echo "3. 运行 cargo build --workspace 验证编译"
echo ""
