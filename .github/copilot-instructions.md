# Copilot Coding Agent Instructions for x-claw

> 本文件由 GitHub Copilot Coding Agent 在派单时自动注入。所有规约的**真理来源**是
> [`AGENTS.md`](../AGENTS.md)（仓库根），本文件只是它的精简检查清单。

## 1. 强制阅读顺序（开任何 PR 之前）

1. [`AGENTS.md`](../AGENTS.md) — 中文规约总入口
2. 本 issue body 顶部的 `**Source**:` 链接（指向 `docs/plans/architecture-refactor/...`） — **必须 `read_file` 至少一次**，禁止只看标题猜内容
3. 涉及红线的 ADR：[`docs/plans/architecture-refactor/adr-112-compatibility-evaluation.md`](../docs/plans/architecture-refactor/adr-112-compatibility-evaluation.md)、
   [`adr-113-hook-engine-unification.md`](../docs/plans/architecture-refactor/adr-113-hook-engine-unification.md) +
   issue body **关联 ADR** 字段中的全部条目 — 同样必须 `read_file`
4. 触及 crate 的 `lib.rs` 模块文档

> **Sources read 自检**（PR 必填）：在 PR body 的 `## Sources read` 段落中
> 逐条回声你实际打开过的文件路径 + § 标题。**只写"已阅"会被视为未读**。
> 模板见 [`.github/pull_request_template.md`](pull_request_template.md)。


## 2. 不要碰的红线

- 标了 `adr-redline` label 的 issue **禁止 agent 自动操作** — 留给人类
- 不要新增 `unwrap()` / `expect()` / `panic!()` 在生产代码（`scripts/check_no_panics.py` 会拦）
- 不要写补丁式代码：不在已有函数尾部追加 `if` 分支，不复制粘贴逻辑，不靠 flag 切换大块代码
- 不要绕过编译期红线（如 `dasclaw_hooks::count_hook_systems()` 的 `const _: () = assert!(...)`）
- 不要改 `--no-verify` / 不要 `git push --force` 到共享分支

## 3. 必跑的本地管线（按顺序）

```bash
# 1. 只 check 本轮改动的 crate（1–3 分钟）
cargo check -p <touched-crate> --tests

# 2. 跑改动范围内的 nextest（< 1 分钟）
cargo nextest run -p <touched-crate>

# 3. fmt + check_no_panics
cargo fmt --all
python3.12 scripts/check_no_panics.py --base origin/xClaw

# 4. crate 级 clippy（不跑 workspace）
cargo clippy --no-deps -p <touched-crate> --all-targets -- -D warnings
```

完整 `cargo build` / workspace clippy / heavy integration **由 CI 兜底**，本地不要跑（10+ 分钟）。

## 4. PR 必填项

- 标题前缀对应 issue 类别：`feat(<area>):` / `fix(...):` / `docs(...):` 等
- Body 必须含 `Closes #<issue>`（缺它 board 状态机不会推进，会被 CI 拦下）
- Body 必须含 4 块：**背景/目标**、**改动范围**、**非目标（What's NOT in this PR）**、**验证（命令 + 结果）**
- stacked PR 必须额外注明：base 分支不是 `xClaw`、merge 顺序、"先看 #X 再看本 PR"

## 5. 测试纪律

- TDD：先红测，再实现，再重构
- 失败路径与成功路径同等重要；安全功能要 **Fail-Safe**，不允许 Fail-Open
- 测试命名：需求测试用 `req_<module>_<id>_<desc>`，安全用 `test_security_<attack>`
- 用 `cargo nextest run` 不用 `cargo test`

## 6. 复用优先于新建

新增 crate / 模块前先 `semantic_search` 是否已有等价实现（codex / claw-code / ironclaw 三方）。
**否定性结论**（"X 没有 Y"、"缺失 Y"）必须三层验证：semantic_search → vscode_listCodeUsages → rg。

## 7. 完成后

- 跑 skills 管线：`code-quality-audit` → `code-simplifier` → `code-review-expert`（详见 `AGENTS.md` Skills 强制使用规范）
- 在 PR 描述贴上 skill 自查产出
- 若已完成一个闭环 milestone，按 `AGENTS.md` 会话轮换原则写 handoff
