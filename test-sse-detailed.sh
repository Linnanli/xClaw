#!/bin/bash

TOKEN="59c7c863fa5bd3eeffc94533cd70a3393251c3ada49a226146a5a61ba62d6743"
THREAD_ID="test-$(date +%s)"

echo "=== Detailed SSE Test ==="
echo ""

# 启动 SSE 连接并保存输出
echo "Starting SSE connection..."
curl -N "http://localhost:3000/api/chat/events?token=$TOKEN" > /tmp/sse-output.txt 2>&1 &
SSE_PID=$!

sleep 2

# 发送消息
echo "Sending message..."
curl -s -X POST "http://localhost:3000/api/chat/send" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{\"content\": \"请用中文回复：你好\", \"thread_id\": \"$THREAD_ID\"}"

echo ""
echo "Waiting for response (15 seconds)..."
sleep 15

# 停止 SSE
kill $SSE_PID 2>/dev/null || true

echo ""
echo "=== SSE Output ==="
cat /tmp/sse-output.txt
echo ""
echo "=== End ==="
