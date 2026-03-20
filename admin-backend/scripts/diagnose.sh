#!/bin/bash

# 快速诊断脚本
# 检查所有服务的状态和常见问题

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo ""
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}Admin Backend 诊断工具${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""

# 1. 检查 Docker
echo -e "${BLUE}[1/7] 检查 Docker${NC}"
if command -v docker &> /dev/null; then
    echo -e "${GREEN}✅ Docker 已安装${NC}"
    
    if docker ps &> /dev/null; then
        echo -e "${GREEN}✅ Docker daemon 正在运行${NC}"
    else
        echo -e "${RED}❌ Docker daemon 未运行${NC}"
        echo -e "${YELLOW}   解决方案: 启动 Docker Desktop 应用${NC}"
    fi
else
    echo -e "${RED}❌ Docker 未安装${NC}"
    echo -e "${YELLOW}   解决方案: 访问 https://www.docker.com/get-started 安装 Docker${NC}"
fi
echo ""

# 2. 检查 PostgreSQL 容器
echo -e "${BLUE}[2/7] 检查 PostgreSQL 容器${NC}"
if docker ps 2>/dev/null | grep -q admin-backend-postgres; then
    echo -e "${GREEN}✅ PostgreSQL 容器正在运行${NC}"
    
    # 检查数据库是否就绪
    if docker exec admin-backend-postgres pg_isready -U postgres &> /dev/null; then
        echo -e "${GREEN}✅ PostgreSQL 数据库已就绪${NC}"
    else
        echo -e "${YELLOW}⚠️  PostgreSQL 容器运行中但数据库未就绪${NC}"
    fi
else
    echo -e "${RED}❌ PostgreSQL 容器未运行${NC}"
    echo -e "${YELLOW}   解决方案: cd admin-backend && ./scripts/start-db.sh${NC}"
fi
echo ""

# 3. 检查后端服务
echo -e "${BLUE}[3/7] 检查后端服务 (端口 3000)${NC}"
if lsof -i :3000 &> /dev/null; then
    echo -e "${GREEN}✅ 后端服务正在运行 (端口 3000)${NC}"
    
    # 测试健康检查端点
    if curl -s http://localhost:3000/health &> /dev/null; then
        echo -e "${GREEN}✅ 后端健康检查通过${NC}"
    else
        echo -e "${YELLOW}⚠️  后端端口已监听但健康检查失败${NC}"
    fi
else
    echo -e "${RED}❌ 后端服务未运行${NC}"
    echo -e "${YELLOW}   解决方案: cd admin-backend && cargo run${NC}"
fi
echo ""

# 4. 检查前端服务
echo -e "${BLUE}[4/7] 检查前端服务 (端口 5174)${NC}"
if lsof -i :5174 &> /dev/null; then
    echo -e "${GREEN}✅ 前端服务正在运行 (端口 5174)${NC}"
else
    echo -e "${RED}❌ 前端服务未运行${NC}"
    echo -e "${YELLOW}   解决方案: cd admin-backend/frontend && npm run dev${NC}"
fi
echo ""

# 5. 检查 Rust 环境
echo -e "${BLUE}[5/7] 检查 Rust 环境${NC}"
if command -v cargo &> /dev/null; then
    RUST_VERSION=$(rustc --version 2>/dev/null | awk '{print $2}')
    echo -e "${GREEN}✅ Rust 已安装 (版本: $RUST_VERSION)${NC}"
else
    echo -e "${RED}❌ Rust 未安装${NC}"
    echo -e "${YELLOW}   解决方案: 访问 https://rustup.rs 安装 Rust${NC}"
fi
echo ""

# 6. 检查 Node.js 环境
echo -e "${BLUE}[6/7] 检查 Node.js 环境${NC}"
if command -v node &> /dev/null; then
    NODE_VERSION=$(node --version 2>/dev/null)
    echo -e "${GREEN}✅ Node.js 已安装 (版本: $NODE_VERSION)${NC}"
    
    if command -v npm &> /dev/null; then
        NPM_VERSION=$(npm --version 2>/dev/null)
        echo -e "${GREEN}✅ npm 已安装 (版本: $NPM_VERSION)${NC}"
    else
        echo -e "${RED}❌ npm 未安装${NC}"
    fi
else
    echo -e "${RED}❌ Node.js 未安装${NC}"
    echo -e "${YELLOW}   解决方案: 访问 https://nodejs.org 安装 Node.js${NC}"
fi
echo ""

# 7. 检查日志文件
echo -e "${BLUE}[7/7] 检查日志文件${NC}"
if [ -f /tmp/admin-backend.log ]; then
    BACKEND_LOG_SIZE=$(wc -l < /tmp/admin-backend.log)
    echo -e "${GREEN}✅ 后端日志存在 ($BACKEND_LOG_SIZE 行)${NC}"
    echo -e "${YELLOW}   查看: tail -f /tmp/admin-backend.log${NC}"
else
    echo -e "${YELLOW}⚠️  后端日志不存在${NC}"
fi

if [ -f /tmp/admin-frontend.log ]; then
    FRONTEND_LOG_SIZE=$(wc -l < /tmp/admin-frontend.log)
    echo -e "${GREEN}✅ 前端日志存在 ($FRONTEND_LOG_SIZE 行)${NC}"
    echo -e "${YELLOW}   查看: tail -f /tmp/admin-frontend.log${NC}"
else
    echo -e "${YELLOW}⚠️  前端日志不存在${NC}"
fi
echo ""

# 总结
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}诊断完成${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""

# 提供快速修复建议
echo -e "${YELLOW}快速修复建议:${NC}"
echo ""

# Docker 未运行
if ! docker ps &> /dev/null; then
    echo -e "${RED}1. 启动 Docker Desktop${NC}"
    echo "   macOS: 打开 Applications 文件夹，启动 Docker 应用"
    echo "   Windows: 打开开始菜单，搜索并启动 Docker Desktop"
    echo ""
fi

# PostgreSQL 未运行
if ! docker ps 2>/dev/null | grep -q admin-backend-postgres; then
    echo -e "${RED}2. 启动 PostgreSQL 数据库${NC}"
    echo "   cd admin-backend && ./scripts/start-db.sh"
    echo ""
fi

# 后端未运行
if ! lsof -i :3000 &> /dev/null; then
    echo -e "${RED}3. 启动后端服务${NC}"
    echo "   cd admin-backend && cargo run"
    echo ""
fi

# 前端未运行
if ! lsof -i :5174 &> /dev/null; then
    echo -e "${RED}4. 启动前端服务${NC}"
    echo "   cd admin-backend/frontend && npm run dev"
    echo ""
fi

echo -e "${GREEN}或者使用一键启动脚本:${NC}"
echo "  cd admin-backend && ./scripts/start-admin.sh"
echo ""

echo -e "${YELLOW}查看详细故障排查指南:${NC}"
echo "  cat admin-backend/TROUBLESHOOTING.md"
echo ""
