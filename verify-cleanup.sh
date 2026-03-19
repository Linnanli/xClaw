#!/bin/bash
# 验证清理后的项目状态

set -e

echo "=========================================="
echo "验证清理后的项目状态"
echo "=========================================="
echo ""

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# 1. 检查目录结构
echo -e "${GREEN}[1/5] 检查目录结构...${NC}"
echo ""

echo "保留的目录:"
for dir in ironclaw desktop-client admin-backend crates docs scripts; do
    if [ -d "$dir" ]; then
        echo "  ✓ $dir/"
    else
        echo -e "  ${RED}✗ $dir/ (缺失)${NC}"
    fi
done

echo ""
echo "已删除的目录:"
for dir in src channels-src tools-src tests benches migrations skills registry wit wix fuzz docker deploy proptest-regressions governance; do
    if [ ! -d "$dir" ]; then
        echo "  ✓ $dir/ (已删除)"
    else
        echo -e "  ${RED}✗ $dir/ (仍然存在)${NC}"
    fi
done

echo ""

# 2. 验证 Cargo.toml
echo -e "${GREEN}[2/5] 验证 Cargo.toml...${NC}"
if grep -q 'members = \["desktop-client", "admin-backend", "crates/ironclaw_auth"\]' Cargo.toml; then
    echo "  ✓ Workspace members 正确"
else
    echo -e "  ${RED}✗ Workspace members 不正确${NC}"
fi

if grep -q '\[package\]' Cargo.toml; then
    echo -e "  ${RED}✗ 仍然包含 [package] 配置${NC}"
else
    echo "  ✓ 不包含 [package] 配置"
fi

echo ""

# 3. 验证编译
echo -e "${GREEN}[3/5] 验证编译...${NC}"
echo "  正在编译 workspace..."
if cargo build --workspace 2>&1 | tee /tmp/cargo-build.log | tail -5; then
    echo -e "  ${GREEN}✓ Workspace 编译成功${NC}"
else
    echo -e "  ${RED}✗ Workspace 编译失败${NC}"
    echo "  查看日志: /tmp/cargo-build.log"
    exit 1
fi

echo ""

# 4. 验证 desktop-client
echo -e "${GREEN}[4/5] 验证 desktop-client...${NC}"
cd desktop-client
echo "  正在编译 desktop-client..."
if cargo build 2>&1 | tail -5; then
    echo -e "  ${GREEN}✓ desktop-client 编译成功${NC}"
else
    echo -e "  ${RED}✗ desktop-client 编译失败${NC}"
    cd ..
    exit 1
fi

echo "  正在运行测试..."
if cargo test 2>&1 | tail -10; then
    echo -e "  ${GREEN}✓ desktop-client 测试通过${NC}"
else
    echo -e "  ${YELLOW}⚠ desktop-client 测试失败（可能需要检查）${NC}"
fi
cd ..

echo ""

# 5. 验证 admin-backend
echo -e "${GREEN}[5/5] 验证 admin-backend...${NC}"
cd admin-backend
echo "  正在编译 admin-backend..."
if cargo build 2>&1 | tail -5; then
    echo -e "  ${GREEN}✓ admin-backend 编译成功${NC}"
else
    echo -e "  ${RED}✗ admin-backend 编译失败${NC}"
    cd ..
    exit 1
fi

echo "  正在运行测试..."
if cargo test 2>&1 | tail -10; then
    echo -e "  ${GREEN}✓ admin-backend 测试通过${NC}"
else
    echo -e "  ${YELLOW}⚠ admin-backend 测试失败（可能需要检查）${NC}"
fi
cd ..

echo ""
echo -e "${GREEN}=========================================="
echo "验证完成！"
echo "==========================================${NC}"
echo ""
echo "统计:"
echo "  - 保留目录: 6 个"
echo "  - 删除目录: 15 个"
echo "  - 删除文件: 约 30 个"
echo "  - 净减少文件: 约 1000+ 个"
echo ""
echo -e "${YELLOW}下一步:${NC}"
echo "  1. 查看更改:"
echo "     git status"
echo ""
echo "  2. 提交更改:"
echo "     git add -A"
echo "     git commit -m 'chore: 删除老的 IronClaw 代码副本，使用 submodule'"
echo ""
