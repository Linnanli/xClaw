#!/bin/bash

# 快速启动 PostgreSQL 数据库
# 用于独立启动数据库服务

set -e

# 获取脚本所在目录
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
ADMIN_BACKEND_ROOT="$( cd "$SCRIPT_DIR/.." && pwd )"

# 加载共享函数
source "$SCRIPT_DIR/common.sh"

# 检查 Docker 依赖
check_docker_dependencies

# 启动数据库
if start_postgres_db "$ADMIN_BACKEND_ROOT"; then
    log_section "启动完成"
    show_database_info
else
    exit 1
fi
