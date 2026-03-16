#!/bin/bash

TOKEN="59c7c863fa5bd3eeffc94533cd70a3393251c3ada49a226146a5a61ba62d6743"
THREAD_ID="test-thread-$(date +%s)"

echo "=== Testing SSE Flow ==="
echo "Thread ID: $THREAD_ID"
echo ""

# 1. 创建线程
echo "1. Creating thread..."
curl -s -X POST "http://localhost:3000/api/chat/threads" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{\"title\": \"Test Thread\"}" | jq .

echo ""

# 2. 启动 SSE 连接（后台）
echo "2. Starting SSE connection..."
curl -N "http://localhost:3000/api/chat/events?token=$TOKEN" &
SSE_PID=$!
echo "SSE PID: $SSE_PID"

sleep 2

# 3. 发送消息
echo ""
echo "3. Sending message..."
curl -s -X POST "http://localhost:3000/api/chat/send" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{\"content\": \"Hello, this is a test\", \"thread_id\": \"$THREAD_ID\"}" | jq .

echo ""
echo "4. Waiting for SSE events (10 seconds)..."
sleep 10

# 4. 停止 SSE 连接
echo ""
echo "5. Stopping SSE connection..."
kill $SSE_PID 2>/dev/null || true

echo ""
echo "=== Test Complete ==="
