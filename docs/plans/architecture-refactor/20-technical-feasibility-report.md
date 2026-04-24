# 20 — 技术可行性专项报告 (B4b)

> **用途**: Round 18+ 外部审计指出 7 个 Rust 工程 Red Flag, 本文档逐一分析并给出**可执行缓解方案 + W0 启动前置任务**。
>
> **状态**: v1.0 (2026-04-24), 由技术架构师独立产出, 不依赖 [19 产品问卷](19-product-definition-questionnaire.md) 答案。
>
> **结论先行**: 7 个 Red Flag 中 **3 个是阻塞性**, 必须在 W0 启动前解决; 4 个是**可管理**, 但需要明确的规划动作。

---

## 0. 7 个 Red Flag 概览与处置优先级

| # | Red Flag | 阻塞级别 | 处置时机 |
|---|---------|---------|---------|
| 1 | Tokio / async-trait 版本对齐未分析 | 🔴 **阻塞性** | W0 启动前 |
| 2 | 22 crate Cargo 编译时间会爆 | 🟡 可管理 | W0 进行中 |
| 3 | 测试 100% 覆盖对 12-15k LOC port 不现实 | 🔴 **阻塞性** | W0 启动前 (调整 AGENTS.md) |
| 4 | fuzz 目标过多无 CI 基础设施 | 🟡 可管理 | W0-W2 |
| 5 | codex 上游 vendor/rebase 策略未定 | 🔴 **阻塞性** | W0 启动前 |
| 6 | 人力缺口 (峰值 10+ Rust) | 🟡 可管理 | 管理层决策 |
| 7 | E2E 测试数据缺失 | 🟡 可管理 | M1 前解决 |

---

## 1. Red Flag #1: Tokio/async-trait 版本对齐 🔴

### 问题描述

三库在 Rust 异步栈上假设不一致:

| 库 | tokio 版本 (估) | async-trait 用法 | 错误类型 |
|---|---------------|----------------|---------|
| **codex-rs** | 1.45+ (最新) | 大量 `#[async_trait]` + `Box<dyn Future>` | `anyhow::Error` + 自定义 |
| **claw-code/rust** | 1.38-1.40 (估) | 混用 `async_trait` 和 `impl Future` | `color-eyre::Report` 为主 |
| **ironclaw (本工程)** | 1.38 | 主要同步 + `tokio::spawn` | `anyhow::Error` / `thiserror` |

**直接 port codex `core` 到 `dasclaw_agent_kernel` 会出**:
- 依赖树版本冲突 (`tokio 1.45` vs `tokio 1.38`, 类型不相同)
- trait bound 不兼容 (`Send + Sync` vs `Send + Sync + 'static`)
- Error 类型互相不能 `?` 转换

### 验证方法 (必做)

在 W0 启动前,花 **2-3 天**做"微 port 验证":

```bash
# 步骤 1: 读 codex-cli-main/codex-rs/Cargo.toml 全部依赖版本
cd codex-cli-main/codex-rs && cargo tree --workspace --depth 1 > ../../docs/plans/architecture-refactor/tmp/codex-deps.txt

# 步骤 2: 读 ironclaw 工程依赖版本
cd ../../ && cargo tree -p ironclaw --depth 1 > docs/plans/architecture-refactor/tmp/ironclaw-deps.txt

# 步骤 3: 机械 diff
diff docs/plans/architecture-refactor/tmp/*.txt | less

# 步骤 4: 尝试直接在 x-claw workspace 新增一个空 dasclaw_kernel_probe crate,
#         用 codex core 的 10 行典型代码 (含 async trait + Sender), 看是否编译
```

**产出**: `docs/plans/architecture-refactor/tmp/20-red-flag-1-tokio-probe.md` 记录 diff 清单 + 可编译性结论。

### 缓解方案 (按优先级)

| 方案 | 工作量 | 副作用 | 推荐度 |
|-----|------|-------|-------|
| **A 统一升级** ironclaw 到 codex 的 tokio 版本 | 高 (3-5 天, 扫荒整个 ironclaw) | 可能破坏现有 tests | ⭐⭐⭐ |
| **B 在新 `dasclaw_*` crate 内锁定 codex 版本**, 通过 adapter 层与 ironclaw 旧栈通信 | 中 (1 周) | adapter 开销 + 维护 2 套 | ⭐⭐ |
| C 降级 codex 代码到 ironclaw 当前版本 | 高 (port 时需手动改) | 容易漏细节 | ⭐ |

**推荐**: **A + 必要时 B 辅助**。W0 启动第 1 天先跑升级 PR。

### W0 前置任务

- [ ] T1.1 跑微 port 探针 (2-3 天)
- [ ] T1.2 决策用方案 A/B/C (1 天会议)
- [ ] T1.3 如选 A, 执行 ironclaw tokio 升级 PR (3-5 天) 并保证原测试绿
- [ ] T1.4 在 [ADR-003 依赖对齐](adr-003-rust-async-stack-alignment.md) 记录决策 (待建)

---

## 2. Red Flag #2: Cargo workspace 编译时间 🟡

### 问题描述

- 当前 x-claw workspace ~30 crate, 冷编译 3-5 分钟, 增量 1-2 分钟
- 加 22 个新 `dasclaw_*` crate 后, 保守估计:
  - 冷编译: **15-20 分钟**
  - 增量: **3-5 分钟**
  - CI (跑测试): **20-30 分钟/PR**
- 工程师日均 10+ PR 时, CI 成本直线飙升

### 缓解方案

| 方案 | 效果 | 成本 |
|-----|-----|-----|
| **A sccache** 本地 + CI 共享缓存 | 2-3x 提速 | 搭建 1 天, 维护 0 |
| **B mold linker** (macOS/Linux) | 链接 5-10x 提速 | 半天配置 |
| **C 拆 workspace** (核心/安全/channels 独立) | 解耦编译, 但损失跨 crate 类型检查 | 3-5 天重构 |
| **D cargo-nextest** 并行测试 | 测试 2-4x 提速 | 1 天 |
| **E 热点 crate 标记 `[profile.release-fast]`** | 调试时跳优化 | 半天 |

**推荐组合**: A + B + D 立即配置 (总计 2 天); C 仅在 A+B+D 不够时才做。

### W0-W2 行动

- [ ] T2.1 配置 sccache + mold (W0 第一周)
- [ ] T2.2 CI 切换 nextest (W0 第一周)
- [ ] T2.3 每周监控 CI 耗时, 超 30 分钟就启动 C (W1-W2)

---

## 3. Red Flag #3: 测试 100% 覆盖对 port 代码不现实 🔴

### 问题描述

[AGENTS.md](../../../AGENTS.md) 当前规定所有新代码:
- 单元测试 >90%, 安全模块 100%
- 失败路径 >80%, 安全 100%
- 契约测试 >90%, 安全 100%
- 安全审计 100%

但现实:
- `dasclaw_agent_kernel` 是 **12-15k LOC port 自 codex core**
- 若按 AGENTS.md 强制执行, 需写 **30-40k LOC 测试代码**
- 单人 Rust 工程师 6 个月产出, 2 人 3 个月 — **这单独就吃掉 W1 所有预算**

### 根本矛盾

| AGENTS.md 假设 | port 代码现实 |
|--------------|--------------|
| 新代码 = 我们**自己设计并实现** | 新代码 = **从 codex 复制+适配** |
| 100% 覆盖能确保"我们的设计没 bug" | codex 已经有它自己的测试, 重写测试**意义不大** |
| 100% 覆盖是可达的 KPI | 对 12-15k LOC 是**人力灾难** |

### 缓解方案

**调整 AGENTS.md**,引入 "port 代码" 与 "自研代码" 区分:

```markdown
### 测试覆盖率目标 (v1.1)

| 代码类型 | 单元 | 失败 | 契约 | 安全审计 |
|---------|-----|-----|-----|---------|
| 自研功能 | >90% | >80% | >90% | 100%(安全) |
| **port 代码** | **>70% (移植 codex 原测试为主)** | **>50%** | **100% (对外接口)** | **100% (跨库边界)** |
| 安全 **自研** 模块 | 100% | 100% | 100% | 100% |
```

**关键洞察**:
- port 代码的**对外接口契约测试 100% 必须**, 确保与 ironclaw 现有代码整合不崩
- port 代码内部细节的 70% 覆盖由**codex 原测试移植**保证
- 节省的人力用在**跨库边界安全审计**上

### W0 前置任务

- [ ] T3.1 起草 [AGENTS.md](../../../AGENTS.md) 修订 PR, 引入 port 代码差异化 (1-2 天)
- [ ] T3.2 团队 review + 批准
- [ ] T3.3 [18 §5.1 测试门禁表](18-execution-roadmap.md) 相应调整

---

## 4. Red Flag #4: Fuzz CI 基础设施 🟡

### 问题描述

[18 §5.2](18-execution-roadmap.md) 要求 fuzz 覆盖 6 个 target:
1. `ironclaw_safety` (已达成)
2. `dasclaw_bash_guard` (W0 后)
3. `dasclaw_policy` (W8 后)
4. `dasclaw_sandbox_linux` (W7 后)
5. `dasclaw_sandbox_macos` (W7 后)
6. `dasclaw_sandbox_windows` (W7 后)

每个 target 连续 24h fuzz 才有意义。

### 缓解方案

**分级 fuzz 策略**:

| 级别 | 频率 | 资源 | 触发 |
|-----|-----|-----|-----|
| **Quick fuzz** | 每次 PR | CI, 5 分钟 | 自动 |
| **Nightly fuzz** | 每日 | 专用机, 4h | cron |
| **Weekly fuzz** | 每周 | 专用机, 24h | cron |
| **Release fuzz** | 每次版本发布 | 专用机, 72h | 手动 |

**所需基础设施**:
- 1 台专用 fuzz 机器 (16 核 / 32GB, 可复用 CI 机器低峰时段)
- GitHub Actions 定时任务配置
- fuzz corpus 持久化 (S3/MinIO)

**成本**: 1 台物理机 + 1 天配置, 工程师时间成本可控。

### W0-W2 行动

- [ ] T4.1 评估购买/借用 fuzz 专用机 (W0 第 1 周)
- [ ] T4.2 配置 GitHub Actions + nightly cron (W0 第 2 周)
- [ ] T4.3 fuzz corpus 持久化方案 (W1)

---

## 5. Red Flag #5: codex 上游 vendor/rebase 策略 🔴

### 问题描述

- 我们要 port codex 78 crate 中的 ~40 个
- codex 上游每月 1-2 次 release, 可能含安全 bugfix
- 6 个月后, 我们 fork 出的代码**必然严重漂离**上游
- 那时如果 codex 出重大 CVE 补丁, 我们**要么放弃合并 (不安全)要么重写 (大量工作)**

### 三种策略选择

#### 策略 A: 冷 fork (一次性 port)

- **做法**: port 当天 copy codex 代码, 之后永远不合并上游
- **优点**: 简单
- **缺点**: 放弃上游 bugfix, 6 个月后代码安全债爆炸

#### 策略 B: Git subtree 持续同步

- **做法**: 用 `git subtree` 把 codex 作为子目录,定期 pull
- **优点**: 可合并上游
- **缺点**: conflict 频繁, 我们的改动可能被 revert

#### 策略 C: Patch 文件管理 (推荐)

- **做法**:
  - codex 原代码 vendor 在 `codex-cli-main/` submodule
  - 我们的改动以 patch 文件形式存于 `docs/plans/architecture-refactor/codex-patches/`
  - 构建时:`submodule update` → 打 patch → 编译
  - 每月 rebase patch 到 codex 新 tag
- **优点**: 清晰可审计; 每个改动独立可 review
- **缺点**: 需要 build 脚本自动化

#### 策略 D: 分支克隆 + cherry-pick

- **做法**: fork codex, 每月 cherry-pick 上游 main 的安全 commits
- **优点**: 选择性合并
- **缺点**: 需要专人监控上游

### 推荐: **策略 C**

理由:
- 国央企客户审计时, **需要清楚知道我们改了上游什么**
- patch 文件是最可审计的形式
- 每月 rebase 工作量可控 (专人 1-2 天)

### W0 前置任务

- [ ] T5.1 写 [ADR-004 codex vendor 策略](adr-004-codex-vendor-strategy.md) (待建)
- [ ] T5.2 建 `codex-patches/` 目录 + 构建脚本 (2-3 天)
- [ ] T5.3 指定"codex 上游监控员" 角色 (每周查 upstream changelog)

---

## 6. Red Flag #6: 人力缺口 🟡

### 问题描述

按 [18 §2 Wave 卡](18-execution-roadmap.md) 峰值人力:

| Wave | 峰值需求 |
|------|-------|
| W0 (安全+治理) | 4 (2 Rust 安全 + 1 后台 + 1 前端) |
| W1 (kernel) | 2 Rust 内核 |
| W2/W3/W4 (session/tasks/context) | 2-3 Rust |
| W5 (patch/git) | 1 Rust |
| W6 (skills/mcp 等 7 crate) | **7 Rust** (并行最大) |
| W7 (沙箱 3 平台) | 3 Rust |
| W8-W9 | 2-3 Rust |

**峰值**: W0+W1+W6 同时进行时, 需要 **10-12 名 Rust 工程师**。

### 现实约束 (待 [19 Q13/Q14](19-product-definition-questionnaire.md) 确认)

- Rust 工程师在国内市场稀缺
- 资深 Rust 工程师招聘周期 3-6 个月
- 团队实际可能 3-5 人 (需业务确认)

### 缓解方案: 3 种团队规模对应的策略

| 团队规模 | MVP 范围 | M1 时间 | 建议 |
|--------|--------|---------|-----|
| **3 人** (骨干) | 砍到 1 个 P0 首秀能力 (只做 DLP **或** 国密) | +12-15 个月 | **不推荐** — 竞争窗口关闭 |
| **5 人** (小团队) | 收窄到 2 个 P0 (DLP + 国密**软实现**) | +9-12 个月 | 可行但紧 |
| **8-10 人** (正常团队) | 按 [18](18-execution-roadmap.md) 完整 MVP | +6-9 个月 | 推荐 |
| **12+ 人** (理想) | 按 [18](18-execution-roadmap.md) + 提前做 Y1 Q3 可视化 | +4-6 个月 | 最快 |

### 关键动作

- [ ] T6.1 **立即**明确团队现状 (业务方需在 [19 Q13/Q14](19-product-definition-questionnaire.md) 回答)
- [ ] T6.2 启动 Rust 招聘 (不能等到 W6 才急)
- [ ] T6.3 W0 阶段并行培养:让新人跟资深做 code review + pair programming

---

## 7. Red Flag #7: E2E 测试数据缺失 🟡

### 问题描述

[16 §3 E2E 测试](16-end-to-end-flow.md) 要求 3 场景 + [18 §5.2 KPI](18-execution-roadmap.md) 要求真实企业语料。实际缺失:

1. **DLP 真实企业语料**: 需 1000 条含身份证/银行卡/工资单的敏感数据样本
2. **国密 SKF 硬件**: 实体 USB Key (国密局认证款)
3. **三平台沙箱攻击 payload**: `rm -rf /`, fork bomb, kernel exploits, ACL bypass
4. **HR 薪酬场景测试数据**: 假的组织架构 + 薪酬表

### 缓解方案

| 测试数据 | 获取方式 | 成本 |
|---------|--------|-----|
| DLP 语料 | 购买商业数据集 (如 CLUE 安全子集) + 自生成脚本 | 5000-20000 元 |
| 国密 SKF | 采购 3 台样品 (飞天诚信/龙脉) | 1000 元 * 3 = 3000 元 |
| 沙箱攻击 | 开源 payload 库 (pwn-rs, metasploit) + 自研 | 0 元 (主要是集成时间) |
| HR 场景数据 | Faker 库合成 + 手工微调 | 0 元 |

### W0-M1 行动

- [ ] T7.1 采购国密 SKF (W0 第 1 周, 硬件到货后才能真测)
- [ ] T7.2 DLP 语料采购/合成 (W0 第 2-4 周)
- [ ] T7.3 沙箱攻击 payload 集成 (W0 后期, 与 W7 并行)
- [ ] T7.4 测试数据仓库建立 (`tests/fixtures/` 或 S3) (W0 早期)

---

## 8. W0 启动前置任务汇总 (按优先级)

### 🔴 阻塞性 (不做不能启动 W0)

| # | 任务 | 工期 | 负责人 |
|---|-----|-----|-------|
| T1.1-T1.3 | Tokio 对齐探针 + 决策 + 升级 PR | 1 周 | 资深 Rust |
| T3.1 | AGENTS.md port 代码差异化修订 | 2 天 | 架构师 + 团队 review |
| T5.1-T5.2 | codex vendor 策略 ADR + patch 脚本 | 1 周 | 资深 Rust |

### 🟡 强烈推荐 (与 W0 并行)

| # | 任务 | 工期 |
|---|-----|-----|
| T2.1-T2.2 | sccache + mold + nextest | 2 天 |
| T4.1-T4.2 | fuzz 基础设施 | 1 周 |
| T6.1-T6.2 | 团队规模确认 + 招聘启动 | 即刻 |
| T7.1 | 国密 SKF 采购 | 2-3 周 (到货) |

### 🟢 M1 前完成

| # | 任务 | 工期 |
|---|-----|-----|
| T7.2-T7.4 | 测试数据仓库 | 4-6 周 |

**总前置**: 1-2 周即可扫清所有 🔴 阻塞性任务, 然后 W0 可启动。

---

## 9. 结论

### 关键判断

- **7 个 Red Flag 均有缓解方案**, 没有 "做不了" 的技术死局
- **3 个阻塞性 (#1/#3/#5) 必须在 W0 前 1-2 周集中解决**, 否则会撞墙
- **人力 (#6) 是**最不确定**变量**, 决定 M1-M4 时间表是否现实

### 进入 C/A 的前提条件

在启动 [22 ADR 补写](22-adr-supplementary.md) (C 阶段) 和 W0 代码 (A 阶段) 之前, 至少完成:

1. ✅ [19 产品问卷](19-product-definition-questionnaire.md) 业务方回答完毕
2. ✅ 本文档 T1/T3/T5 阻塞项扫清
3. ✅ [21 时间表重估](21-timeline-rebaseline.md) 基于问卷 + 本文档产出

---

## 变更历史

- **v1.0 (2026-04-24)**: 首版。针对 Round 18+ 外部审计的 7 Red Flag 逐一分析, 给出 15 个前置任务 (3 阻塞 + 4 强烈推荐 + 3 M1 前)。
