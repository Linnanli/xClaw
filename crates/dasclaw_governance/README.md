# dasclaw_governance

> Self-governance 6-pack: policy / recovery / trust / branch_lock / stale / green.

W4 阶段 crate 骨架。每个治理模块通过 cargo feature 单独开启，**默认全部关闭**。

## 模块表

| 模块 | Issue | Cargo feature | 描述 |
|---|---|---|---|
| `policy_engine` | #65 | `policy_engine` | 策略引擎核心（claw-code 端口） |
| `recovery_recipes` | #66 | `recovery_recipes` | 7 类 FailureScenario + RecoveryStep |
| `trust_resolver` | #67 | `trust_resolver` | OAuth + worktree + allowlist |
| `branch_lock` | #68 | `branch_lock` | 分支保护策略 |
| `stale_base` | #69 | `stale_base` | base 落后检测 |
| `stale_branch` | #69 | `stale_branch` | 分支陈旧检测 |
| `green_contract` | #70 | `green_contract` | level 0-3 绿契约 |
| `lane_events` | #71 | `lane_events` | governance hook 联动 |

## 用法

```toml
[dependencies]
# 默认（仅 trait surface，无任何模块体）
dasclaw_governance = { path = "../crates/dasclaw_governance" }

# 单独启用某模块（实现就绪后）
dasclaw_governance = { path = "../crates/dasclaw_governance", features = ["policy_engine"] }

# 集成测试一次启全
dasclaw_governance = { path = "../crates/dasclaw_governance", features = ["full"] }
```

## 验证

```bash
cargo check -p dasclaw_governance                      # 默认 features
cargo check -p dasclaw_governance --features full      # 全开
cargo nextest run -p dasclaw_governance                # 包含 default-off 守卫测试
```

## 红线

- `default = []` 由 `req_governance_64_default_features_off` 测试保护，禁止默认开启任何模块。
- 实现 PR（#65-#71）必须只动自己 feature 对应的 `.rs` 文件 + 必要的 `[dependencies]`，不得触碰 `[features]` 默认值。

## 设计参考

- `docs/plans/architecture-refactor/31-target-architecture.md` §4
- `docs/plans/architecture-refactor/32-execution-plan.md` W4
