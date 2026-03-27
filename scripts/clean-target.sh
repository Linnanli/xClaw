#!/usr/bin/env bash
# 清理 Rust 编译缓存
# 用法:
#   ./scripts/clean-target.sh          # 只清增量缓存（快，保留 rlib）
#   ./scripts/clean-target.sh --all    # 完全清理 target/（慢，彻底）

set -e

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$PROJECT_ROOT"

print_size() {
  if [ -d "$1" ]; then
    du -sh "$1" 2>/dev/null | cut -f1
  else
    echo "0"
  fi
}

case "${1:-}" in
  --all)
    echo "完全清理 target/ ..."
    before=$(print_size target)
    cargo clean
    echo "已清理，释放约 ${before}"
    ;;
  *)
    echo "清理增量编译缓存（保留 rlib）..."
    before=$(print_size target)

    rm -rf target/debug/incremental
    rm -rf target/release/incremental

    # cargo-sweep 清理 30 天未使用的旧产物（如已安装）
    if command -v cargo-sweep &>/dev/null; then
      cargo sweep -t 30
    fi

    after=$(print_size target)
    echo "清理完成：${before} → ${after}"
    echo "提示：如需彻底清理，运行 $0 --all"
    ;;
esac
