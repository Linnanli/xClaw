#!/bin/bash
# 在 Lima VM 中运行 E2E 测试
#
# 用法：
#   ./run-in-lima.sh              # 运行全部测试
#   ./run-in-lima.sh routines     # 仅定时任务测试
#   ./run-in-lima.sh logs         # 仅日志测试

set -e

VM_NAME="ironclaw-e2e"
SPEC_FILTER="${1:-}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

if ! command -v limactl &>/dev/null; then
  echo "❌ Lima 未安装。运行：brew install lima"
  exit 1
fi

# 创建 VM（如果不存在）
if ! limactl list | grep -q "^${VM_NAME}"; then
  echo "🚀 首次运行，创建 Lima VM（约 5-10 分钟）..."
  limactl start "${SCRIPT_DIR}/lima-e2e.yaml" --name "${VM_NAME}"
fi

# 启动 VM（如果未运行）
VM_STATUS=$(limactl list | grep "^${VM_NAME}" | awk '{print $2}')
if [ "${VM_STATUS}" != "Running" ]; then
  echo "▶️  启动 Lima VM..."
  limactl start "${VM_NAME}"
fi

echo "🧪 在 Lima VM 中运行 E2E 测试..."
echo "   项目路径: ${PROJECT_ROOT}"
echo "   测试过滤: ${SPEC_FILTER:-全部}"
echo ""

limactl shell "${VM_NAME}" -- bash -c "
  set -e
  source \"\$HOME/.cargo/env\" 2>/dev/null || true
  export DISPLAY=:99
  export CARGO_TARGET_DIR='${PROJECT_ROOT}/target'

  # 启动虚拟显示（如果没在跑）
  xdpyinfo -display :99 &>/dev/null 2>&1 || (Xvfb :99 -screen 0 1280x720x24 & sleep 1)

  cd '${PROJECT_ROOT}/desktop-client/e2e'

  if [ ! -d node_modules ]; then
    npm install
  fi

  if [ -n '${SPEC_FILTER}' ]; then
    npm run 'test:${SPEC_FILTER}'
  else
    npm test
  fi
"

echo ""
echo "✅ 测试完成"
