# 34. 资深架构师评审报告

> **角色**：资深 Rust 系统架构师 + 严格批判者
> **评审对象**：30-architecture-truth.md / 31-target-architecture.md / 32-execution-plan.md / 33-feasibility-and-validation.md
> **评审方法**：8 维度评分 + 反模式排查 + 反对决策清单
> **综合评分**：**3.4 / 5**（⚠️ 可推进但必须修订）

---

## 0. 核心结论一句话

> **思路正确，但 4 处软肋会在 W6-W8 翻出来**：依赖版本管理太松散、"可复用"是纸面承诺、时间估算过度乐观、4 处架构反模式埋藏。建议**先做 1 周 W0 PoC** 再开 W1。

---

## 1. 8 维度评分

| 维度 | 分数 | 状态 | 改进优先级 |
|------|------|------|----------|
| A. 边界清晰性 | 3.5/5 | ⚠️ 可接受但需强化 | 🔴 P1 |
| B. 可复用性 | 3/5 | 🔴 被高估 | 🔴 P1 |
| C. 决策完备性 | 4/5 | ✅ 较好 | 🟡 P2 |
| D. 执行可行性 | 3.5/5 | ⚠️ 时间被低估 | 🔴 P1 |
| E. 风险识别 | 3/5 | ⚠️ 遗漏高风险 | 🔴 P1 |
| F. 测试策略 | 3/5 | ⚠️ 数字没验证 | 🟡 P2 |
| G. non-goal 清单 | 4.5/5 | ✅ 很清楚 | 🟢 P3 |
| H. 架构反模式 | 3.5/5 | ⚠️ 隐患多 | 🔴 P1 |
| **综合** | **3.4/5** | **⚠️ 可推进但需修订** | — |

---

## 2. 必须修改清单（5 条，阻塞 W1 启动）

### M1. ADR-105 改写：L5 (ironclaw) 不许直接 import
- **现状**：x_claw_core 通过 path dep 直接 `use ironclaw::llm`
- **问题**：x_claw_core 无法独立发布，必须跟 ironclaw 版本同步 → "可独立部署"是假的
- **改法**：
  - 新建 `crates/x_claw_core_traits`（仅 trait 定义）
  - x_claw_core 依赖 traits
  - desktop-client 应用层作为"胶水"，把 ironclaw 接口适配到 trait
  - ironclaw 改成 `git tag` 或锁定版本号 `ironclaw = "=0.26.5"`，不再 path dep

### M2. 32 Wave 时间重评（6-8 月 → 8-10 月）
| Wave | 原估 | 修订 | 理由 |
|------|------|------|------|
| W1 基线收敛 | 3 周 | **4 周** | x_claw_agent + ironclaw_engine v2 是"两个半成品缝合"，需加完整 loop 验证子任务 |
| W2 三平台沙箱 | 4 周 | **5 周（2 周 trait + 3 周并行实现）** | landlock kernel 兼容、JobObject Home 版兼容 |
| W4 治理 6 件套 | 3 周 | **4 周** | policy_engine + green_contract 单拎集成测试 |
| W6 客户端整合 | 3 周 | **4 周** | 14 IPC 模块 + DLP 集成 + Cypress E2E + 行为基线对比 |

### M3. 新增"Hook 引擎执行规范"文档
明确以下契约（W3 阻塞依赖）：
```
BeforeInbound 决策优先级：Reject > Modify > Pass
任何 Reject → 整个 loop 立即停止
同 lifecycle 多 hook：按注册顺序串行；前一个 output 是后一个 input
short-circuit：decision != Pass 时不再传给下一个
```

### M4. DLP 入口唯一化决策
- **现状**：30/31 文档允许 DLP 在 IPC 层 + Hook 层"二选一"未定
- **问题**：两个入口会导致重复计数、延迟统计混乱、脱敏结果不一致
- **必做**：W1 评审时锁定一个，另一个删除。建议 **Hook 层（BeforeInbound）**，符合 L3 收敛原则

### M5. governance 6 件套"原子性" feature flag
- **现状**：6 个模块各自 feature flag
- **问题**：内部强耦合（recovery → policy_engine、branch_lock ↔ stale、trust ← policy_engine），独立开关是假象
- **必做**：W4 启动前做"governance 内部依赖 DAG 分析"，结果二选一：
  - 改成单一 `enable_self_governance` bool
  - 或证明真的可独立开关（需提供 cargo build 全关闭场景下能编译的证据）

---

## 3. 反模式清单（W1 必须排查）

| 编号 | 反模式 | 影响 | 阻塞 Wave |
|------|--------|------|----------|
| H1 | x_claw_core 是上帝 crate（直接依赖 tools/hooks/governance/mcp/execpolicy） | 编译时间螺旋、单包 bug 拖累全局、测试 setup 复杂 | W1 |
| H2 | L5 (ironclaw) 被 L3 强依赖（缺 trait adapter） | 无法独立发布、版本必须锁同步 | W1 |
| H3 | x_claw_hooks trait 缺契约文档 | 安全漏洞（拒绝被绕过）、性能问题（无 short-circuit） | W3 |
| H4 | DLP 双入口（IPC 层 + Hook 层并存） | 数据上报重复、延迟统计错乱、脱敏不一致 | W1 |
| H5 | governance 6 件套 feature flag 是 facade 假象 | 关闭后 trait 调用点编译失败或运行时 panic | W4 |

**强制 W1 子任务（W0 PoC，1 周）**：
1. ✅ 检查 x_claw_core dependencies，不含 ironclaw 具体模块
2. ✅ 检查 x_claw_hooks 链有 short-circuit + priority 实现
3. ✅ 检查 DLP 调用点唯一
4. ✅ 检查 governance crate 在 feature 全关闭时能编译

---

## 4. 我会反对的设计决策（3 条）

### 反对 #1：Desktop-client "零回退" 是过度承诺
- **反对的位置**：31 §5 "下列模块零改动，只更新引用"
- **理由**：DLP / PolicySync / DataReporter 从 L2 IPC 移到 L3 后，**调用链和执行时机必定改变**
  - DLP 现在是请求入口立即执行；新架构可能延迟到 agent loop 内部
  - 脱敏失败的拒绝策略可能从"立即拒绝"变成"跳过+log"
  - 审计上报 timing 改变
- **建议**：改成"**向下兼容 + 30 天观察期**"
  - W6 发布后 2-4 周外测 + 指标对标
  - DLP 误报率涨幅 > 5% 或脱敏准确率跌幅 > 2% 立即 rollback

### 反对 #2：codex 三平台 sandbox 等价性假设
- **反对的位置**：32 W2 "port 三平台进程沙箱"语调上等价
- **理由**：三平台 threat model 不等价
  - Linux landlock：kernel 4.17+ syscall 级隔离（fine-grained）
  - macOS seatbelt：.sbpl 规则，app-level 隔离
  - Windows JobObject：进程数/内存/CPU 限制，**无 syscall 级过滤**
- **建议**：W2 验收必须明确"相同用例三平台拦截率"
  ```
  Test: 工具 open("/etc/shadow")
  - Linux: ✓ landlock 拦截
  - macOS: ✓ seatbelt 拦截
  - Windows: ? JobObject 看不到 Unix 路径
  ```
  接受差异 → 文档化；不接受 → Windows 特殊处理（W2 内附加任务）

### 反对 #3：claw-code 6 件套打包成 x_claw_governance
- **反对的位置**：31 文档统一打包到 x_claw_governance crate
- **理由**：
  - 6 件套是为**编译器辅助**（merge/rebase/escalate）设计的，是**特定领域**
  - desktop-client 是**通用对话 agent**，"可选"是空话（政企客户必启用 = 强制）
  - 启用 governance 改变整个 runtime 语义，非 runtime 开关可控
- **建议**：
  - 改为独立 crate `crates/x_claw_agent_governance`（不并入 core）
  - desktop-client 对 governance 走 **cargo feature 编译时决策**，不是 runtime flag
  - 不编译进二进制 → 运行时 0 开销

---

## 5. 建议讨论清单（5 条，非阻塞但需对齐）

1. **ironclaw-main 版本锁定**：`{ git, tag = "v0.26.x" }` vs `= "0.26.5"` vs 定期评估升级窗口？需新增 ADR-108
2. **沙箱隔离等级**：无副作用工具（如列文件）是否需要沙箱？trait 是否加 `IsolationLevel { Strict, Moderate, None }`？
3. **MultiAgent + Hook 交互**：SubAgent 是否触发 Hook？主 agent 与 SubAgent 的 hook 链是否独立？
4. **性能监控内置化**：W1 起每个 Wave CI 输出性能指标做趋势图，不要等 W8 才发现回退
5. **历史文档清理时机**：旧的 11/16/17/18/27 在 W9 前做版本矩阵说明

---

## 6. 遗漏的高风险（补充到 33 文档 R1-R7 之后）

| 风险 | 等级 | 说明 |
|------|------|------|
| **R8：ironclaw-main 主分支漂移** | 🔴 高 | v0.27 改 Agent trait 时 x-claw 是否 follow？版本锁定策略缺失 |
| **R9：Desktop "零回退" 过度乐观** | 🔴 高 | DLP 调用链变长，延迟+节流策略必定改变 |
| **R10：Windows 沙箱隔离不等价** | 🔴 高 | JobObject 无 syscall 过滤，威胁模型不一致 |
| **R11："复用 60-70%" 是经验数字** | 🟡 中 | 未基于 LOC 抽样统计 |
| **R12：policy_engine 与 DLP 决策冲突** | 🟡 中 | 两个决策引擎，分工边界缺失 |
| **R13：Hooks lifecycle 顺序在三方 port 后不一致** | 🟡 中 | codex hook schema 顺序 vs ironclaw 6 lifecycle 顺序 merge 后谁优先？ |

**风险矩阵补充列**：
- 每个风险新增 **"缓解责任方"** 列（R8→Team Lead、R10→Infra Engineer 等）
- "复用 60-70%" 改为可度量：**W1 PoC 抽样 5 个 craw-code 测试文件，统计原样复用 vs 改写比例**

---

## 7. 测试策略具体改进（替换 33 文档 §测试矩阵）

### 7.1 性能基线必须有数值
```
Baseline (current desktop-client R2.1):
  LLM first token: ~850ms (P50), ~1200ms (P99)
  Tool execution: <100ms (sandbox-less)

Target (x-claw post-refactor):
  LLM first token: <935ms (P50), <1320ms (P99)  [< +10%]
  Tool execution: <150ms (with sandbox)         [+50ms sandbox overhead cap]
```

### 7.2 失败路径"100% 覆盖"清单（具体化）
- ✓ Sandbox 启动失败 → 工具调用被拒
- ✓ Hook BeforeInbound 拒绝 → loop 终止
- ✓ DLP 脱敏失败 → 消息被拒（fail-safe，非 fail-open）
- ✓ LLM timeout + 全 failover 失败 → 用户收明确拒绝
- ✓ governance policy_engine 拒绝 → 工具不执行
- ✓ apply_patch 校验失败 → 不写盘

### 7.3 新增"多库集成 E2E"
不只是 cypress，还要验证 governance + hooks + DLP **在同一流程中协作不冲突**

---

## 8. non-goal 措辞改进（替换 ADR-104）

### 8.1 拆成两类
- **永久 non-goal**：Realtime WebRTC、AWS-auth、ChatGPT API proxy
- **MVP (v1.0) 不做，v2.0 重估**：TUI、App-Server、Cloud Tasks

### 8.2 新增"主动拒绝"清单
- ❌ 把所有编码逻辑改成可视化界面（编码就是写 Rust/Python）
- ❌ 支持插件自定义 sandbox 规则（sandbox 是硬防线）
- ❌ 任意 Hook 实现都能 Pass（必须遵守优先级规则）

---

## 9. 项目经理视角的"启动条件"

### ✅ 可以立即启动 W0 PoC（1 周），前提：
- [ ] 用户确认 ADR-105 改成 trait adapter
- [ ] 用户确认 DLP 入口唯一化方案（建议 Hook 层）
- [ ] 用户确认 governance 改用 cargo feature 编译时决策
- [ ] 反模式 H1-H5 排查清单纳入 W1 子任务

### ❌ 暂不启动 W2-W4，等 W0+W1 完成后看 x_claw_core 实情再调整后续

> 多花 1-2 周做 W0，省掉后面 3-4 周的反模式重构。**Early detect, early fix**。

---

## 10. 修订 Wave 时间表（建议）

| Wave | 原估 | 评审建议 | 备注 |
|------|------|---------|------|
| **W0 反模式 PoC** | — | **+1 周** | 评审新增，验证 H1-H5 |
| W1 基线收敛 | 3 周 | 4 周 | 加完整 loop 验证 |
| W2 三平台沙箱 | 4 周 | 5 周 | 2w trait + 3w 并行实现 |
| W3 hooks + apply_patch | 4 周 | 4 周 | 不变 |
| W4 治理 6 件套 | 3 周 | 4 周 | 加内部集成测试 |
| W5 MCP + execpolicy | 4 周 | 4 周 | 不变 |
| W6 客户端整合 | 3 周 | 4 周 | 加行为基线对比 |
| W7 可观测+features+identity | 3 周 | 3 周 | 不变 |
| W8 验证硬化 | 4 周 | 4 周 | 不变 |
| W9 文档收尾 | 2 周 | 2 周 | 不变 |
| **合计** | **30 周（≈7 月）** | **35 周（≈8.5 月）** | +1 W0 +5 中间 |

---

## 11. 后续动作

1. **本文档作为 4 份新文档（30-33）的伴生评审**，归档于 architecture-refactor/
2. **不擅自修改 30/31/32/33** — 上述改动涉及 ADR 重写，必须用户拍板
3. **W0 PoC 任务清单**作为第一个待审 Decision Point（D7）补充进 31 §8

> 评审就到这。话不好听，但比 W6 才发现问题强。
