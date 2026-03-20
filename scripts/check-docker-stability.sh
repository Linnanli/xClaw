#!/bin/bash

# Docker 稳定性检查脚本
# 用于诊断 Docker daemon 是否稳定运行

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() {
    echo -e "${GREEN}✅ $1${NC}"
}

log_error() {
    echo -e "${RED}❌ $1${NC}"
}

log_warn() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

echo ""
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}Docker 稳定性检查${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""

# 检查次数
CHECK_COUNT=10
CHECK_INTERVAL=2
FAILED_COUNT=0

log_info "开始检查 Docker daemon 稳定性（${CHECK_COUNT} 次，间隔 ${CHECK_INTERVAL} 秒）..."
echo ""

for i in $(seq 1 $CHECK_COUNT); do
    echo -n "检查 $i/$CHECK_COUNT: "
    
    if docker info > /dev/null 2>&1; then
        echo -e "${GREEN}✓ 正常${NC}"
    else
        echo -e "${RED}✗ 失败${NC}"
        FAILED_COUNT=$((FAILED_COUNT + 1))
    fi
    
    if [ $i -lt $CHECK_COUNT ]; then
        sleep $CHECK_INTERVAL
    fi
done

echo ""
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}检查结果${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""

SUCCESS_COUNT=$((CHECK_COUNT - FAILED_COUNT))
SUCCESS_RATE=$((SUCCESS_COUNT * 100 / CHECK_COUNT))

echo "成功: $SUCCESS_COUNT/$CHECK_COUNT ($SUCCESS_RATE%)"
echo "失败: $FAILED_COUNT/$CHECK_COUNT"
echo ""

if [ $FAILED_COUNT -eq 0 ]; then
    log_info "Docker daemon 运行稳定 ✨"
    echo ""
    echo "可以安全运行 start-all.sh"
    exit 0
elif [ $FAILED_COUNT -lt 3 ]; then
    log_warn "Docker daemon 偶尔不稳定"
    echo ""
    echo "建议："
    echo "  1. 重启 Docker Desktop"
    echo "  2. 检查系统资源（内存、CPU）"
    echo "  3. 查看 Docker Desktop 日志"
    exit 1
else
    log_error "Docker daemon 非常不稳定"
    echo ""
    echo "问题严重，建议："
    echo ""
    echo "1. 完全重启 Docker Desktop："
    echo "   osascript -e 'quit app \"Docker\"'"
    echo "   sleep 10"
    echo "   open -a Docker"
    echo "   sleep 60"
    echo ""
    echo "2. 检查 Docker Desktop 日志："
    echo "   tail -100 ~/Library/Containers/com.docker.docker/Data/log/vm/dockerd.log"
    echo ""
    echo "3. 如果问题持续，重置 Docker Desktop："
    echo "   Docker Desktop → Troubleshoot → Reset to factory defaults"
    echo ""
    echo "4. 查看详细修复指南："
    echo "   cat DOCKER_CONNECTION_FIX.md"
    echo ""
    exit 1
fi
