建议工作流：
sccache

只改前端：直接 cd desktop-client/src-ui && npm run dev，浏览器访问 localhost:5173
只改 Rust：cargo build -p desktop-client --lib 单独编译，确认无错后再启动
需要完整 Tauri：才用 start-desktop.sh

日常改代码 → cargo build -p desktop-client --lib，只重编改动的 crate，几秒到几十秒
磁盘满了 → clean-target.sh，清掉增量缓存 + 旧产物
clean 后重编 → sccache 命中缓存，速度比无缓存快很多
定期自动清理 → 设 cron 0 3 * * * cd ~/Documents/code/x-claw && cargo sweep --time 3
核心逻辑：不要再用 cargo clean 或 cargo sweep --time 0，用 cargo sweep --time 3 做温和清理就够了。


# 在项目根目录（不是 ironclaw 子目录）
cargo build -p desktop-client --lib

---

## Enterprise sandbox 现状（三平台一致 fail-closed）

桌面端在 `enterprise_mode = true` 时，三平台均走内核级 sandbox，缺失即拒绝 spawn，不再退化为 user-space 检查。

| 平台 | 内核层 | 状态 | 主要依据 |
|---|---|---|---|
| macOS | Seatbelt (`sandbox-exec`) + `read_only_subpaths` SBPL DENY | ✅ 内核强制 | ADR-141 §1.1 |
| Linux | Landlock V3 + seccomp + `read_only_subpaths` DENY | ✅ 内核强制（Wave-C1c） | ADR-144 |
| Windows | Job Object + AppContainer + DACL DENY (`additional_deny_write_paths`) | ✅ 内核强制（Wave-C1b PR #426） | ADR-141 §1.1 / ADR-129 |

要点：

- 内核 sandbox 未就绪时，`dasclaw_exec` 直接返回 `SandboxError`，**不会**回退到直接 exec。
- 软模式（`enterprise_allow_userspace_carveouts`）已在 Wave-C1c 移除（PR #448），无 opt-out。
- `enterprise_mode` 是配置层字段（`dasclaw_sandbox::SandboxBackendConfig`），不暴露给终端用户切换。

详细 ADR 路径：[`docs/plans/architecture-refactor/adr-141-windows-enterprise-sandbox-support.md`](../docs/plans/architecture-refactor/adr-141-windows-enterprise-sandbox-support.md) / [`adr-144-linux-writable-root-kernel-enforcement.md`](../docs/plans/architecture-refactor/adr-144-linux-writable-root-kernel-enforcement.md)。
