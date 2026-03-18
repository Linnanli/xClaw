#!/bin/bash
# 最终架构重构脚本 - 使用 IronClaw 中的 ironclaw_safety

set -e

echo "=========================================="
echo "IronClaw Suite 最终架构重构"
echo "=========================================="
echo ""

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${YELLOW}重要发现：${NC}"
echo "ironclaw_safety 已被 IronClaw 上游接受！"
echo "我们将使用 IronClaw 中的版本，删除重复的 crates/ironclaw_safety/"
echo ""

read -p "是否继续? (y/n) " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "已取消"
    exit 0
fi

echo ""
echo -e "${YELLOW}阶段 1: 备份${NC}"

# 备份配置文件
cp Cargo.toml Cargo.toml.backup
echo -e "${GREEN}✓ 已备份 Cargo.toml${NC}"

# 备份 ironclaw_safety
if [ -d "crates/ironclaw_safety" ]; then
    mv crates/ironclaw_safety crates/ironclaw_safety.backup
    echo -e "${GREEN}✓ 已备份 crates/ironclaw_safety${NC}"
fi

echo ""
echo -e "${YELLOW}阶段 2: 创建新的工作空间配置${NC}"

cat > Cargo.toml << 'EOF'
[workspace]
members = [
    "desktop-client",
    "admin-backend",
    "crates/ironclaw_auth",
]

# IronClaw 作为独立的工作空间，通过路径依赖使用
# ironclaw_safety 使用 IronClaw 中的版本
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
ironclaw_safety = { path = "ironclaw/crates/ironclaw_safety" }
EOF

echo -e "${GREEN}✓ 新的 Cargo.toml 已创建${NC}"

echo ""
echo -e "${YELLOW}阶段 3: 验证配置${NC}"

echo "检查工作空间配置..."
if cargo metadata --format-version 1 > /dev/null 2>&1; then
    echo -e "${GREEN}✓ 工作空间配置有效${NC}"
else
    echo -e "${RED}✗ 工作空间配置无效${NC}"
    echo "恢复备份..."
    cp Cargo.toml.backup Cargo.toml
    if [ -d "crates/ironclaw_safety.backup" ]; then
        mv crates/ironclaw_safety.backup crates/ironclaw_safety
    fi
    exit 1
fi

echo ""
echo -e "${YELLOW}阶段 4: 编译验证${NC}"

echo "运行 cargo check..."
if cargo check --workspace 2>&1 | tee /tmp/cargo-check.log; then
    echo -e "${GREEN}✓ 编译检查通过${NC}"
else
    echo -e "${RED}✗ 编译检查失败${NC}"
    echo "查看日志: /tmp/cargo-check.log"
    echo ""
    echo "恢复备份..."
    cp Cargo.toml.backup Cargo.toml
    if [ -d "crates/ironclaw_safety.backup" ]; then
        mv crates/ironclaw_safety.backup crates/ironclaw_safety
    fi
    exit 1
fi

echo ""
echo "=========================================="
echo -e "${GREEN}重构成功完成！${NC}"
echo "=========================================="
echo ""
echo "已完成的工作:"
echo "1. ✓ 删除重复的 crates/ironclaw_safety"
echo "2. ✓ 更新工作空间配置"
echo "3. ✓ 验证编译通过"
echo ""
echo "下一步操作:"
echo "1. 运行 cargo build --workspace 完整编译"
echo "2. 运行 cargo test --workspace 运行测试"
echo "3. 测试 Desktop Client 和 Admin Backend 功能"
echo "4. 提交代码: git add . && git commit -m 'refactor: 使用 IronClaw 中的 ironclaw_safety'"
echo ""
echo "备份文件位置:"
echo "- Cargo.toml.backup"
echo "- crates/ironclaw_safety.backup"
echo ""
echo "如果一切正常，可以删除备份:"
echo "  rm Cargo.toml.backup"
echo "  rm -rf crates/ironclaw_safety.backup"
echo ""
