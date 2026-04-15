#!/bin/bash

set -euo pipefail

print_header() {
    printf '%-7s %-20s %-20s %-8s %s\n' "当前" "名称" "分支" "状态" "路径"
}

worktree_state() {
    local worktree_path="$1"
    local worktree_status

    if [[ ! -d "$worktree_path" ]]; then
        printf 'missing\n'
        return 0
    fi

    worktree_status="$(git -C "$worktree_path" status --porcelain --untracked-files=all 2>/dev/null || true)"
    if [[ -n "$worktree_status" ]]; then
        printf 'dirty\n'
        return 0
    fi

    printf 'clean\n'
}

print_entry() {
    local worktree_path="$1"
    local head_sha="$2"
    local branch_ref="$3"
    local current_mark=""
    local branch_name
    local state

    [[ -z "$worktree_path" ]] && return 0

    if [[ "$worktree_path" == "$PROJECT_ROOT" ]]; then
        current_mark="*"
    fi

    if [[ -n "$branch_ref" ]]; then
        branch_name="${branch_ref#refs/heads/}"
    else
        branch_name="detached@${head_sha:0:8}"
    fi

    state="$(worktree_state "$worktree_path")"
    printf '%-7s %-20s %-20s %-8s %s\n' "$current_mark" "$(basename "$worktree_path")" "$branch_name" "$state" "$worktree_path"
}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

current_path=""
current_head=""
current_branch=""

cd "$PROJECT_ROOT"

print_header
while IFS= read -r line || [[ -n "$line" ]]; do
    if [[ -z "$line" ]]; then
        print_entry "$current_path" "$current_head" "$current_branch"
        current_path=""
        current_head=""
        current_branch=""
        continue
    fi

    case "$line" in
        worktree\ *)
            current_path="${line#worktree }"
            ;;
        HEAD\ *)
            current_head="${line#HEAD }"
            ;;
        branch\ *)
            current_branch="${line#branch }"
            ;;
        detached)
            current_branch=""
            ;;
    esac
done < <(git worktree list --porcelain)

print_entry "$current_path" "$current_head" "$current_branch"