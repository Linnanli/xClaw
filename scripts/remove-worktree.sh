#!/bin/bash

set -euo pipefail

usage() {
    cat <<'EOF'
用法:
  ./scripts/remove-worktree.sh --name <worktree-name> [--path <target-dir>] [--delete-branch] [--force]
  ./scripts/remove-worktree.sh <worktree-name>

说明:
  --name / 第一个位置参数: worktree 名称，默认对应 ../<repo>.worktrees/<worktree-name>
  --path: 直接指定要删除的 worktree 路径
  --delete-branch: 删除 worktree 后，一并删除该 worktree 当前检出的本地分支
    --force: 即使 worktree 有未提交变更也继续删除

示例:
  ./scripts/remove-worktree.sh audit
  ./scripts/remove-worktree.sh --name ui-polish --delete-branch
  ./scripts/remove-worktree.sh --path ../x-claw.worktrees/fix-login --force
EOF
}

require_option_value() {
    local option_name="$1"
    local option_value="${2:-}"

    if [[ -z "$option_value" || "$option_value" == -* ]]; then
        echo "$option_name 需要一个参数值" >&2
        usage >&2
        exit 1
    fi
}

deinit_submodules() {
    if [[ ! -f "$TARGET_PATH/.gitmodules" ]]; then
        return 0
    fi

    echo "[1/4] 反注册子模块"
    git -C "$TARGET_PATH" submodule deinit -f --all
}

ensure_safe_to_remove() {
    local worktree_status

    worktree_status="$(git -C "$TARGET_PATH" status --porcelain --untracked-files=all)"
    if [[ -z "$worktree_status" || "$FORCE_REMOVE" == "true" ]]; then
        return 0
    fi

    echo "worktree 中存在未提交改动，请先提交/清理，或重新执行并加上 --force:" >&2
    echo "$worktree_status" >&2
    exit 1
}

resolve_target_path() {
    if [[ -n "$TARGET_PATH" ]]; then
        printf '%s\n' "$TARGET_PATH"
        return 0
    fi

    printf '%s/%s.worktrees/%s\n' "$(cd "$PROJECT_ROOT/.." && pwd)" "$REPO_NAME" "$WORKTREE_NAME"
}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO_NAME="$(basename "$PROJECT_ROOT")"
WORKTREE_NAME=""
TARGET_PATH=""
DELETE_BRANCH="false"
FORCE_REMOVE="false"

while [[ $# -gt 0 ]]; do
    case "$1" in
        -n|--name)
            require_option_value "$1" "${2:-}"
            WORKTREE_NAME="$2"
            shift 2
            ;;
        -p|--path)
            require_option_value "$1" "${2:-}"
            TARGET_PATH="$2"
            shift 2
            ;;
        --delete-branch)
            DELETE_BRANCH="true"
            shift
            ;;
        --force)
            FORCE_REMOVE="true"
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        --)
            shift
            break
            ;;
        -*)
            echo "未知参数: $1" >&2
            usage >&2
            exit 1
            ;;
        *)
            break
            ;;
    esac
done

if [[ -z "$WORKTREE_NAME" && $# -ge 1 ]]; then
    WORKTREE_NAME="$1"
    shift
fi

if [[ $# -gt 0 ]]; then
    echo "参数过多: $*" >&2
    usage >&2
    exit 1
fi

if [[ -z "$WORKTREE_NAME" && -z "$TARGET_PATH" ]]; then
    echo "必须提供 worktree 名称或路径" >&2
    usage >&2
    exit 1
fi

TARGET_PATH="$(resolve_target_path)"

if [[ ! -d "$TARGET_PATH" ]]; then
    echo "worktree 不存在: $TARGET_PATH" >&2
    exit 1
fi

BRANCH_NAME="$(git -C "$TARGET_PATH" branch --show-current 2>/dev/null || true)"

cd "$PROJECT_ROOT"

ensure_safe_to_remove
deinit_submodules

echo "[2/4] 移除 worktree: $TARGET_PATH"
git worktree remove --force "$TARGET_PATH"

echo "[3/4] 清理 worktree 元数据"
git worktree prune

if [[ "$DELETE_BRANCH" == "true" && -n "$BRANCH_NAME" ]]; then
    echo "[4/4] 删除分支: $BRANCH_NAME"
    git branch -D "$BRANCH_NAME"
else
    echo "[4/4] 完成"
fi
