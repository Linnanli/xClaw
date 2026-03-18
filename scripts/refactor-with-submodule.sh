#!/bin/bash
# 基于现有 Submodule 的架构重构脚本

set -e

echo "=========================================="
echo "IronClaw Suite 架构重构（基于 Submodule）"
echo "=========================================="
echo ""

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${YELLOW}阶段 1: 检查当前状态${NC}"

# 检查 ironclaw submodule
if [ ! -d "ironclaw/.git" ]; then
    echo -e "${RED}错误: ironclaw submodule 未初始化${NC}"
    echo "请运行: git submodule update --init --recursive"
    exit 1
fi

echo -e "${GREEN}✓ IronClaw submodule 已存在${NC}"

# 检查 ironclaw submodule 状态
cd ironclaw
IRONCLAW_STATUS=$(git status --porcelain)
if [ -n "$IRONCLAW_STATUS" ]; then
    echo -e "${YELLOW}警告: IronClaw submodule 有未提交的修改${NC}"
    echo "$IRONCLAW_STATUS"
    echo ""
    read -p "是否重置 IronClaw 到干净状态? (y/n) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        git reset --hard HEAD
        echo -e "${GREEN}✓ IronClaw 已重置${NC}"
    fi
fi
cd ..

echo ""
echo -e "${YELLOW}阶段 2: 备份当前配置${NC}"

# 备份 Cargo.toml
cp Cargo.toml Cargo.toml.backup
echo -e "${GREEN}✓ 已备份 Cargo.toml${NC}"

# 备份 desktop-client/Cargo.toml
cp desktop-client/Cargo.toml desktop-client/Cargo.toml.backup
echo -e "${GREEN}✓ 已备份 desktop-client/Cargo.toml${NC}"

# 备份 admin-backend/Cargo.toml
cp admin-backend/Cargo.toml admin-backend/Cargo.toml.backup
echo -e "${GREEN}✓ 已备份 admin-backend/Cargo.toml${NC}"

echo ""
echo -e "${YELLOW}阶段 3: 创建新的工作空间配置${NC}"

cat > Cargo.toml << 'EOF'
[workspace]
members = [
    "desktop-client",
    "admin-backend",
    "crates/ironclaw_auth",
    "crates/ironclaw_safety",
]

# IronClaw 作为独立的工作空间，通过路径依赖使用
resolver = "2"

[workspace.package]
edition = "2024"
rust-version = "1.92"
authors = ["NEAR AI <support@near.ai>"]
license = "MIT OR Apache-2.0"

[workspace.dependencies]
# 共享依赖版本
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
thiserror = "2"
tracing = "0.1"

# IronClaw 核心（作为路径依赖）
ironclaw = { path = "ironclaw" }

# 共享 Crate
ironclaw_auth = { path = "crates/ironclaw_auth" }
ironclaw_safety = { path = "crates/ironclaw_safety" }
EOF

echo -e "${GREEN}✓ 新的 Cargo.toml 已创建${NC}"

echo ""
echo -e "${YELLOW}阶段 4: 验证配置${NC}"

# 检查配置
echo "检查工作空间配置..."
if cargo metadata --format-version 1 > /dev/null 2>&1; then
    echo -e "${GREEN}✓ 工作空间配置有效${NC}"
else
    echo -e "${RED}✗ 工作空间配置无效${NC}"
    echo "恢复备份..."
    cp Cargo.toml.backup Cargo.toml
    exit 1
fi

echo ""
echo -e "${YELLOW}阶段 5: 创建文档${NC}"

# 创建 docs 目录
mkdir -p docs

# 移动文档
if [ -f "IRONCLAW_CORE_MODIFICATIONS.md" ]; then
    cp IRONCLAW_CORE_MODIFICATIONS.md docs/
    echo -e "${GREEN}✓ 已复制核心修改记录${NC}"
fi

if [ -f "ARCHITECTURE_REFACTOR_PLAN_B.md" ]; then
    cp ARCHITECTURE_REFACTOR_PLAN_B.md docs/
    echo -e "${GREEN}✓ 已复制重构计划${NC}"
fi

if [ -f "ARCHITECTURE_REFACTOR_REVISED.md" ]; then
    cp ARCHITECTURE_REFACTOR_REVISED.md docs/
    echo -e "${GREEN}✓ 已复制修订方案${NC}"
fi

echo ""
echo "=========================================="
echo -e "${GREEN}重构准备完成！${NC}"
echo "=========================================="
echo ""
echo "下一步操作:"
echo "1. 检查 Cargo.toml 配置"
echo "2. 运行 cargo check --workspace 验证"
echo "3. 运行 cargo build --workspace 编译"
echo "4. 运行 cargo test --workspace 测试"
echo ""
echo "如果遇到问题，可以恢复备份:"
echo "  cp Cargo.toml.backup Cargo.toml"
echo "  cp desktop-client/Cargo.toml.backup desktop-client/Cargo.toml"
echo "  cp admin-backend/Cargo.toml.backup admin-backend/Cargo.toml"
echo ""
