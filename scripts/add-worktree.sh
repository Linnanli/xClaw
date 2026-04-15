#!/bin/bash

set -euo pipefail

usage() {
    cat <<'EOF'
用法:
    ./scripts/add-worktree.sh --name <worktree-name> [--branch <branch-name>] [--from <base-ref>] [--path <target-dir>] [--no-open]
  ./scripts/add-worktree.sh <worktree-name> [branch-name] [base-ref]

说明:
  --name / 第一个位置参数: worktree 目录名，必填
  --branch / 第二个位置参数: Git 分支名，可选，默认与 worktree 名相同
  --from / 第三个位置参数: 新分支基线，可选，默认 HEAD
  --path: 自定义 worktree 目录，默认创建到 ../<repo>.worktrees/<worktree-name>
    --open: 创建后用 VS Code 新窗口打开 worktree（默认开启）
    --no-open: 创建后不自动打开 VS Code

示例:
  ./scripts/add-worktree.sh audit
  ./scripts/add-worktree.sh --name ui-polish --branch feat/ui-polish --from main
    ./scripts/add-worktree.sh --name fix-login --no-open
  ./scripts/add-worktree.sh fix-login hotfix/login origin/main
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

init_submodules() {
    if [[ ! -f "$PROJECT_ROOT/.gitmodules" ]]; then
        return 0
    fi

    git -C "$TARGET_PATH" submodule sync --recursive

    while read -r submodule_path; do
        [[ -z "$submodule_path" ]] && continue

        local_source="$PROJECT_ROOT/$submodule_path"
        if [[ -d "$local_source/.git" || -f "$local_source/.git" ]]; then
            echo "  - 使用本地子模块对象初始化: $submodule_path"
            git -C "$TARGET_PATH" submodule update --init --reference "$local_source" -- "$submodule_path"
            continue
        fi

        echo "  - 从远端初始化子模块: $submodule_path"
        git -C "$TARGET_PATH" submodule update --init -- "$submodule_path"
    done < <(git config -f "$PROJECT_ROOT/.gitmodules" --get-regexp '^submodule\..*\.path$' | awk '{print $2}')

    git -C "$TARGET_PATH" submodule update --init --recursive
}

open_in_vscode() {
    if [[ "$OPEN_IN_VSCODE" != "true" ]]; then
        return 0
    fi

    if ! command -v code >/dev/null 2>&1; then
        echo "未找到 VS Code 命令行工具 code，跳过自动打开。" >&2
        return 0
    fi

    echo "打开 VS Code: $TARGET_PATH"
    code -n "$TARGET_PATH"
}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO_NAME="$(basename "$PROJECT_ROOT")"
WORKTREE_NAME=""
BRANCH_NAME=""
BASE_REF="HEAD"
TARGET_PATH=""
OPEN_IN_VSCODE="true"

while [[ $# -gt 0 ]]; do
    case "$1" in
        -n|--name)
            require_option_value "$1" "${2:-}"
            WORKTREE_NAME="$2"
            shift 2
            ;;
        -b|--branch)
            require_option_value "$1" "${2:-}"
            BRANCH_NAME="$2"
            shift 2
            ;;
        -f|--from)
            require_option_value "$1" "${2:-}"
            BASE_REF="$2"
            shift 2
            ;;
        -p|--path)
            require_option_value "$1" "${2:-}"
            TARGET_PATH="$2"
            shift 2
            ;;
        --open)
            OPEN_IN_VSCODE="true"
            shift
            ;;
        --no-open)
            OPEN_IN_VSCODE="false"
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

if [[ -z "$BRANCH_NAME" && $# -ge 1 ]]; then
    BRANCH_NAME="$1"
    shift
fi

if [[ "$BASE_REF" == "HEAD" && $# -ge 1 ]]; then
    BASE_REF="$1"
    shift
fi

if [[ $# -gt 0 ]]; then
    echo "参数过多: $*" >&2
    usage >&2
    exit 1
fi

if [[ -z "$WORKTREE_NAME" ]]; then
    echo "必须提供 worktree 名称" >&2
    usage >&2
    exit 1
fi

if [[ -z "$BRANCH_NAME" ]]; then
    BRANCH_NAME="$WORKTREE_NAME"
fi

if [[ -z "$TARGET_PATH" ]]; then
    TARGET_PATH="$(cd "$PROJECT_ROOT/.." && pwd)/${REPO_NAME}.worktrees/${WORKTREE_NAME}"
fi

if [[ ! -d "$PROJECT_ROOT/.git" && ! -f "$PROJECT_ROOT/.git" ]]; then
    echo "项目根目录不是 Git 仓库: $PROJECT_ROOT" >&2
    exit 1
fi

if [[ -e "$TARGET_PATH" && -n "$(find "$TARGET_PATH" -mindepth 1 -maxdepth 1 2>/dev/null)" ]]; then
    echo "目标目录已存在且非空: $TARGET_PATH" >&2
    exit 1
fi

cd "$PROJECT_ROOT"

if git show-ref --verify --quiet "refs/heads/$BRANCH_NAME"; then
    echo "[1/3] 使用已有分支创建 worktree: $BRANCH_NAME"
    git worktree add "$TARGET_PATH" "$BRANCH_NAME"
else
    echo "[1/3] 基于 $BASE_REF 创建新分支并生成 worktree: $BRANCH_NAME"
    git worktree add -b "$BRANCH_NAME" "$TARGET_PATH" "$BASE_REF"
fi

echo "[2/3] 同步并初始化子模块"
if ! init_submodules; then
    echo "子模块初始化失败，已保留 worktree 供排查: $TARGET_PATH" >&2
    exit 1
fi

echo "[3/3] 完成"
echo "worktree 路径: $TARGET_PATH"
echo "进入目录: cd $TARGET_PATH"
open_in_vscode
