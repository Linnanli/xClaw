#!/bin/bash

# Docker 连接问题诊断和修复脚本

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
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

log_section() {
    echo ""
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""
}

log_section "Docker 连接问题诊断"

# 1. 检查 Docker CLI
log_info "检查 Docker CLI..."
if ! command -v docker &> /dev/null; then
    log_error "Docker CLI 未安装"
    exit 1
fi
log_info "Docker CLI 已安装: $(docker --version)"

# 2. 检查 Docker Desktop 进程
log_info "检查 Docker Desktop 进程..."
if ps aux | grep -i "Docker Desktop.app" | grep -v grep > /dev/null; then
    log_info "Docker Desktop 应用正在运行"
else
    log_error "Docker Desktop 应用未运行"
    echo ""
    echo "请启动 Docker Desktop:"
    echo "  打开 Applications 文件夹，双击 Docker.app"
    exit 1
fi

# 3. 检查 Docker daemon
log_info "检查 Docker daemon..."
if docker info &> /dev/null; then
    log_info "Docker daemon 正在运行"
    docker version
    exit 0
fi

log_warn "Docker daemon 未运行或无法连接"

# 4. 检查 Docker context
log_info "检查 Docker context..."
echo "当前 context:"
docker context ls

CURRENT_CONTEXT=$(docker context show)
log_info "当前使用的 context: $CURRENT_CONTEXT"

# 5. 检查 socket 文件
log_info "检查 Docker socket 文件..."

SOCKET_PATHS=(
    "/var/run/docker.sock"
    "$HOME/.docker/run/docker.sock"
)

for SOCKET in "${SOCKET_PATHS[@]}"; do
    if [ -S "$SOCKET" ]; then
        log_info "找到 socket: $SOCKET"
        ls -la "$SOCKET"
    else
        log_warn "socket 不存在: $SOCKET"
    fi
done

# 6. 尝试修复
log_section "尝试修复"

log_info "方案 1: 重启 Docker Desktop"
echo ""
echo "请执行以下步骤:"
echo "  1. 点击菜单栏的 Docker 图标"
echo "  2. 选择 'Quit Docker Desktop'"
echo "  3. 等待 5 秒"
echo "  4. 重新打开 Docker Desktop"
echo "  5. 等待 Docker Engine 启动（菜单栏图标变为正常）"
echo ""
read -p "完成后按 Enter 继续..." 

log_info "验证 Docker daemon..."
if docker info &> /dev/null; then
    log_info "✅ Docker daemon 已恢复正常"
    docker version
    exit 0
fi

log_warn "方案 1 失败，尝试方案 2"

log_info "方案 2: 切换 Docker context"
echo ""
echo "尝试切换到 default context..."
docker context use default 2>&1 || true

log_info "验证 Docker daemon..."
if docker info &> /dev/null; then
    log_info "✅ Docker daemon 已恢复正常"
    docker version
    exit 0
fi

log_warn "方案 2 失败，尝试方案 3"

log_info "方案 3: 重置 Docker Desktop"
echo ""
echo "请执行以下步骤:"
echo "  1. 点击菜单栏的 Docker 图标"
echo "  2. 选择 'Troubleshoot'"
echo "  3. 选择 'Reset to factory defaults'"
echo "  4. 确认重置"
echo "  5. 等待 Docker Desktop 重启"
echo ""
log_warn "注意: 这会删除所有容器、镜像和卷"
echo ""
read -p "完成后按 Enter 继续..." 

log_info "验证 Docker daemon..."
if docker info &> /dev/null; then
    log_info "✅ Docker daemon 已恢复正常"
    docker version
    exit 0
fi

log_section "诊断结果"

log_error "无法修复 Docker 连接问题"
echo ""
echo "建议:"
echo "  1. 完全卸载 Docker Desktop"
echo "  2. 重新下载并安装最新版本"
echo "  3. 下载地址: https://www.docker.com/products/docker-desktop"
echo ""
echo "或者查看 Docker Desktop 日志:"
echo "  ~/Library/Containers/com.docker.docker/Data/log/"
echo ""

exit 1
