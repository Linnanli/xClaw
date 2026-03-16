#!/bin/bash

# 测试 Tauri 客户端 Token 环境变量

# 从后端日志中提取令牌
GATEWAY_URL=$(grep "gateway.*http" /tmp/backend.log | tail -1 | sed 's/.*http/http/')
if [ ! -z "$GATEWAY_URL" ]; then
    export GATEWAY_AUTH_TOKEN=$(echo "$GATEWAY_URL" | sed 's/.*token=//')
    echo "✅ 令牌已提取: ${GATEWAY_AUTH_TOKEN:0:20}..."
    echo ""
    echo "环境变量:"
    echo "  GATEWAY_AUTH_TOKEN=${GATEWAY_AUTH_TOKEN}"
    echo ""
    echo "启动 Tauri 客户端..."
    cd desktop-client
    cargo tauri dev
else
    echo "❌ 无法从后端日志中提取令牌"
    exit 1
fi
