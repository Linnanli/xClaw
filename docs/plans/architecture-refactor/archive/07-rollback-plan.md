# 07 — 回滚与风险控制

> 每个 Phase 都必须"随时可回滚"。本文给出具体策略、测试门禁和应急流程。

---

## 回滚粒度

| Phase | 回滚粒度 | 机制 | 最坏情况恢复时间 |
|-------|---------|------|---------------|
| 1 | 整体 / 单步 | feature flag `VITE_USE_AI_SDK` | 改环境变量 + 重启（<5 分钟） |
| 2 | 整体 | feature flag `rig-llm` / `claw-code-llm` | 重编译 + 发版（<30 分钟） |
| 3 | 整体 | git revert PR | 重编译 + 发版（<1 小时） |

---

## Phase 1 回滚

### 机制

前端用环境变量切换新旧 Runtime：

```tsx
// TauriRuntimeProvider.tsx（过渡期版本）
const USE_AI_SDK = import.meta.env.VITE_USE_AI_SDK === "true";

export function TauriRuntimeProvider(props) {
  if (USE_AI_SDK) {
    return <AiSdkRuntimeProvider {...props} />;
  }
  return <LegacyRuntimeProvider {...props} />;  // 原 1539 行代码
}
```

后端保持**双协议输出**：

- 老路径：现有 Tauri event emit（`append_text` / `approval_needed` 等）
- 新路径：DataStream 帧（通过 `Channel<String>`）

前端根据 flag 订阅不同路径。

### 触发条件

任一回归测试项失败：

- 对话无响应
- 工具调用不触发
- 审批卡片不可见
- 流式渲染异常

### 回滚步骤

```bash
# 1. 关闭 feature flag
echo "VITE_USE_AI_SDK=false" >> desktop-client/src-ui/.env.production

# 2. 重新打包前端
cd desktop-client/src-ui && npm run build

# 3. 重启应用
```

### 清理

Phase 1 稳定运行 2 周后：

- [ ] 删除 `LegacyRuntimeProvider`
- [ ] 删除 feature flag
- [ ] 删除后端老 event 路径
- [ ] Commit 信息标记为 `chore(phase1): cleanup legacy runtime`

---

## Phase 2 回滚

### 机制

Cargo feature flag：

```toml
[features]
default = ["claw-code-llm"]
rig-llm = ["rig-core"]
claw-code-llm = ["claw-code-api"]
```

所有调用点用 `Arc<dyn LlmProvider>`，实现可运行时切换：

```rust
pub fn build_llm(cfg: &Config) -> Arc<dyn LlmProvider> {
    #[cfg(feature = "claw-code-llm")]
    { return Arc::new(ClawCodeLlmProvider::new(cfg.clone())); }
    #[cfg(feature = "rig-llm")]
    { return Arc::new(RigLlmAdapter::new(cfg.clone())); }
}
```

### 触发条件

- 某个 Provider 无法连通
- 流式输出错位（文本不连续、tool call 参数丢失）
- reasoning_content 等字段再次异常
- `scripts/run-real-llm-tests.sh` 任一 Provider 红

### 回滚步骤

```bash
# 1. 切 feature
cargo build --release -p desktop-client --no-default-features --features rig-llm

# 2. 打包发版（紧急热修复通道）

# 3. 在 issue tracker 记录问题 provider + 触发场景
```

### 清理

Phase 2 稳定 **2 周** + 真实模型 E2E **连续 7 天绿** 后：

- [ ] 执行 [`04-phase2-claw-code-api.md`](./04-phase2-claw-code-api.md) Step I
- [ ] 删除 `rig_adapter.rs` 2026 行
- [ ] 删除 `rig-core` Cargo 依赖
- [ ] 删除 `rig-llm` feature
- [ ] Commit: `chore(phase2): remove rig-core`

---

## Phase 3 回滚

### 机制

Phase 3 是大规模代码搬迁，双路径维护成本过高，**不做 feature flag，只做 git revert**。

关键保护：
- 整个 Phase 3 在独立分支 `refactor/phase3-agent-extraction`
- 合并方式：**single squash merge**（回滚是 1 个 commit）
- 合并前 CI 必须包含完整 E2E + 真实 LLM + 安全冒烟

### 触发条件

- agent 运行时行为与 Phase 2 末尾不一致
- 安全 hook 未正确触发（DLP 漏过、审批未弹）
- 首次 claw-code rebase 演练失败（> 1 天人力）
- 新 crate 边界导致循环依赖

### 回滚步骤

```bash
# 1. Revert 合并 commit
git revert -m 1 <phase3-merge-commit>

# 2. 重新编译 + E2E
cargo build --release -p desktop-client
cargo test --all

# 3. 发版
```

### 回滚后补救

回滚不是终点。必须：

1. 把失败场景写成回归测试
2. 在 Phase 3 分支上修复
3. 修复后 **重新合并前**，加这个测试到 CI
4. 合并前再跑 7 天夜间 CI

---

## 跨 Phase 风险矩阵

| 风险 | 概率 | 影响 | 缓解 | 责任人 |
|------|------|------|------|--------|
| AI SDK v4 协议与我们的实现不兼容 | 低 | 高 | Step A 先做协议编码单测对照 Vercel 官方示例 | — |
| Tauri `Channel<String>` 性能不足以承载流式 | 低 | 中 | 压测 1000 msg/s；若不行改为 `channel!` macro + 事件批处理 | — |
| DashScope 在 claw-code api 下仍有兼容问题 | 中 | 高 | Phase 2 Step C 真实模型测试；必要时给 claw-code 提 PR | — |
| 上游 claw-code 某些逻辑依赖特定 tool / plugin 我们没有 | 中 | 中 | Step D 每搬一个文件跑单测 | — |
| SafetyHook 注入漏掉某个调用点 | 中 | 高 | Phase 3 Step D 要求每处 LLM / tool 边界都插 hook，单测覆盖 | — |
| `ironclaw_routines` 与 `x_claw_agent` 产生循环依赖 | 中 | 中 | 强制单向：`routines` depends on `x_claw_agent`，反向不允许 | — |
| 用户配置在 Phase 2 迁移后无法读取 | 中 | 高 | Step D 保留 legacy config test，提供自动迁移 | — |
| 审批策略在 Phase 1 改 UI 后决策错位 | 低 | 高 | Phase 1 Step F 加 E2E 覆盖多种策略组合 | — |

---

## CI 门禁（每个 PR 必过）

### Phase 通用

```yaml
# .github/workflows/refactor-gate.yml（示意）
- name: Rust 编译（所有 feature）
  run: cargo build --all-features --workspace

- name: Rust 测试
  run: cargo test --all-features --workspace

- name: 前端类型检查
  run: cd desktop-client/src-ui && npx tsc --noEmit

- name: 前端 lint
  run: cd desktop-client/src-ui && npm run lint

- name: 安全冒烟
  run: ./scripts/pre-commit-safety.sh

- name: 面板扫描（无 unwrap / 无 panic）
  run: ./scripts/code-quality-gate.sh
```

### Phase 特定

- **Phase 1**：`e2e/ai-sdk-migration.spec.ts` + `e2e/approval-flow.spec.ts`
- **Phase 2**：`./scripts/run-real-llm-tests.sh`（用每日 CI 跑，PR 触发要 key 时跳过）
- **Phase 3**：`cargo test -p x-claw-agent --all-features` + rebase 演练记录

---

## 发版策略

### 预发阶段

每个 Phase 完成后：

1. 打 tag `v0.x.0-phaseN-rc1`
2. 内部试用 3 天
3. 收集问题 → 修 → `rc2`
4. `rc2` 稳定 → tag `v0.x.0-phaseN`

### 生产发布

- **灰度**：前 20% 用户
- **观察窗**：48 小时
- **指标**：崩溃率、LLM 响应时长、工具执行成功率、审批超时率
- **阈值**：任一指标比上一版本恶化 >10% → 暂停灰度

### 紧急回滚

- 全量用户：触发 GitHub Release → 回滚到上个 tag → 自动更新器推送
- 时间 SLA：发现问题到回滚到位 <2 小时

---

## 监控清单

Phase 1 上线后关注：

- [ ] DataStream 帧解析错误数（前端 console）
- [ ] Tauri Channel send 失败率（后端 tracing）
- [ ] Approval 从发出到用户响应的 p50 / p99 延迟
- [ ] 前端渲染卡顿（fps 掉 < 30 的次数）

Phase 2 上线后关注：

- [ ] 每个 Provider 的请求成功率
- [ ] 流式首 token 延迟（TTFT）p50 / p99
- [ ] 工具调用解析失败率（按 Provider 分组）
- [ ] 用户配置自动迁移成功率

Phase 3 上线后关注：

- [ ] Hook 调用次数（确认 safety / approval / sandbox 都被经过）
- [ ] Agent turn 成功率
- [ ] 内存占用（新 crate 边界不应导致泄漏）

---

## 文档责任

每次回滚发生，必须：

1. 在 `docs/plans/architecture-refactor/INCIDENT_LOG.md` 记录：
   - 触发时间
   - 触发条件
   - 回滚耗时
   - 根因分析
   - 改进措施
2. 在对应 Phase 文档更新"已知风险"章节
3. 在下一个 PR 中加回归测试

---

## 最终验收清单（三 Phase 全部完成后）

- [ ] [`rig_adapter.rs`](../../desktop-client/ironclaw/src/llm/rig_adapter.rs) 已删除
- [ ] `rig-core` 依赖移除
- [ ] [`TauriRuntimeProvider.tsx`](../../desktop-client/src-ui/src/app/runtime/TauriRuntimeProvider.tsx) <500 行
- [ ] `agent/*` 24041 行拆解完成
- [ ] `crates/x_claw_agent/` 可独立 `cargo test`
- [ ] `crates/ironclaw_sandbox/` / `ironclaw_secrets/` / `ironclaw_routines/` 存在
- [ ] 首次 claw-code rebase 演练成功（工作量 <1 天）
- [ ] 5 块安全能力全绿（见 [`06-safety-preservation.md`](./06-safety-preservation.md) 验收门禁）
- [ ] 真实 LLM E2E 连续 7 天绿
- [ ] 前端 `useChatRuntime` 接管所有状态，branch 吞审批等 bug 消失
- [ ] 生产灰度 100% 上线稳定 2 周

达成 → 重构完成，进入日常迭代 + 上游 rebase 节奏。
