#!/bin/bash
# 清理老的 IronClaw 代码副本
# 
# 本脚本删除根目录下老的 IronClaw 代码副本，
# 保留 desktop-client, admin-backend, crates/ironclaw_auth 和 ironclaw submodule

set -e

echo "=========================================="
echo "清理老的 IronClaw 代码副本"
echo "=========================================="
echo ""

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# 确认操作
echo -e "${YELLOW}警告: 本操作将删除以下目录和文件:${NC}"
echo ""
echo "目录:"
echo "  - src/"
echo "  - channels-src/"
echo "  - tools-src/"
echo "  - tests/"
echo "  - benches/"
echo "  - migrations/"
echo "  - skills/"
echo "  - registry/"
echo "  - wit/"
echo "  - wix/"
echo "  - fuzz/"
echo "  - docker/"
echo "  - deploy/"
echo "  - proptest-regressions/"
echo "  - governance/"
echo ""
echo "文件:"
echo "  - clippy.toml"
echo "  - codecov.yml"
echo "  - deny.toml"
echo "  - docker-compose.yml"
echo "  - Dockerfile*"
echo "  - release-plz.toml"
echo "  - providers.json"
echo "  - ironclaw.bash/fish/zsh/png"
echo "  - CHANGELOG.md"
echo "  - CLAUDE.md"
echo "  - CONFIG_GUIDE.md"
echo "  - CONTRIBUTING.md"
echo "  - FEATURE_PARITY.md"
echo "  - JOBS_CREATION_FLOW.md"
echo "  - QUICK_START.md"
echo "  - README.ru.md"
echo "  - README.zh-CN.md"
echo ""
echo -e "${YELLOW}保留的目录:${NC}"
echo "  - ironclaw/ (submodule)"
echo "  - desktop-client/"
echo "  - admin-backend/"
echo "  - crates/ironclaw_auth/"
echo "  - docs/"
echo "  - scripts/ (部分文件)"
echo ""

read -p "确认删除? (yes/no): " confirm
if [ "$confirm" != "yes" ]; then
    echo "操作已取消"
    exit 0
fi

echo ""
echo "开始清理..."
echo ""

# 1. 删除老的源码目录
echo -e "${GREEN}[1/4] 删除源码目录...${NC}"
rm -rf src/
rm -rf channels-src/
rm -rf tools-src/
rm -rf tests/
rm -rf benches/
rm -rf migrations/
rm -rf skills/
rm -rf registry/
rm -rf wit/
rm -rf wix/
rm -rf fuzz/
rm -rf docker/
rm -rf deploy/
rm -rf proptest-regressions/
rm -rf governance/
echo "  ✓ 源码目录已删除"

# 2. 删除老的配置文件
echo -e "${GREEN}[2/4] 删除配置文件...${NC}"
rm -f clippy.toml
rm -f codecov.yml
rm -f deny.toml
rm -f docker-compose.yml
rm -f Dockerfile
rm -f Dockerfile.test
rm -f Dockerfile.worker
rm -f release-plz.toml
rm -f providers.json
echo "  ✓ 配置文件已删除"

# 3. 删除老的脚本和文档
echo -e "${GREEN}[3/4] 删除脚本和文档...${NC}"
rm -f ironclaw.bash
rm -f ironclaw.fish
rm -f ironclaw.zsh
rm -f ironclaw.png
rm -f CHANGELOG.md
rm -f CLAUDE.md
rm -f CONFIG_GUIDE.md
rm -f CONTRIBUTING.md
rm -f FEATURE_PARITY.md
rm -f JOBS_CREATION_FLOW.md
rm -f QUICK_START.md
rm -f README.ru.md
rm -f README.zh-CN.md
echo "  ✓ 脚本和文档已删除"

# 4. 清理 scripts/ 目录中的 IronClaw 脚本（保留项目特有的）
echo -e "${GREEN}[4/4] 清理 scripts/ 目录...${NC}"
# 保留以下项目特有的脚本:
# - refactor-with-submodule.sh
# - final-refactor.sh
# - run-e2e-tests.sh
# - test-tauri-token.sh
# - code-quality-gate.sh
# - code-quality-gate.ps1
# - scaffold-tdd-module.sh
# - setup-rust-core.sh
# - start-all.sh
# - migrate-to-suite.sh
# - run-real-llm-tests.sh

# 删除 IronClaw 的脚本（与 ironclaw/scripts/ 重复的）
cd scripts/
for script in *.sh; do
    if [ -f "../ironclaw/scripts/$script" ]; then
        # 检查是否是项目特有的脚本
        if [[ ! "$script" =~ ^(refactor-with-submodule|final-refactor|run-e2e-tests|test-tauri-token|code-quality-gate|scaffold-tdd-module|setup-rust-core|start-all|migrate-to-suite|run-real-llm-tests)\.sh$ ]]; then
            echo "  删除重复脚本: $script"
            rm -f "$script"
        fi
    fi
done
cd ..
echo "  ✓ scripts/ 目录已清理"

echo ""
echo -e "${GREEN}=========================================="
echo "清理完成！"
echo "==========================================${NC}"
echo ""
echo "统计:"
echo "  - 删除目录: 15 个"
echo "  - 删除文件: 约 20+ 个"
echo "  - 净减少文件: 约 1000+ 个"
echo ""
echo -e "${YELLOW}下一步:${NC}"
echo "  1. 验证编译:"
echo "     cargo build --workspace"
echo ""
echo "  2. 验证 desktop-client:"
echo "     cd desktop-client && cargo test"
echo ""
echo "  3. 验证 admin-backend:"
echo "     cd admin-backend && cargo test"
echo ""
echo "  4. 提交更改:"
echo "     git add -A"
echo "     git commit -m 'chore: 删除老的 IronClaw 代码副本，使用 submodule'"
echo ""
