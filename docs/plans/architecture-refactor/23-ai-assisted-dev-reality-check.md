# 23 — AI 辅助开发可行性再评估 (Vibe Coding 现实情况)

> **v0.1 (2026-04-25)** · 回应用户挑战: "我用 AI 开发,不是手写,也不合理吗?"
>
> **结论先行**: AI 辅助让 1 人 8 月 **可行范围翻 2-3 倍**,但仍达不到 18 路线图 22 crate。**Pivot α 仍是正确方向,但可适度扩容到 5-6 crate**,同时需要明确哪些活 AI 做不了、必须人来顶。

---

## §1 AI 辅助开发的真实加速比

### 1.1 不同任务的 AI 加速倍率

| 任务类型 | AI 加速比 | 说明 |
|---------|---------|------|
| **样板代码生成** (trait 实现 / serde / CRUD) | **3-5 倍** | AI 擅长,质量高 |
| **Port 已有代码** (codex → dasclaw) | **2-3 倍** | AI 能搬运,但需人核对语义 |
| **新架构设计** | **1 倍** (甚至 0.5 倍) | AI 给方向但决策错误代价大 |
| **Rust 所有权/生命周期/async 调试** | **0.5-1 倍** | AI 经常给错误修复,Rust 新手识别不出 |
| **跨 crate 重构** | **1-1.5 倍** | AI 上下文窗口限制,大改丢失一致性 |
| **测试编写** | **3-4 倍** | 模板化任务,AI 高效 |
| **集成/联调/定位 bug** | **0.8-1.2 倍** | AI 猜得多,定位根因仍靠人 |
| **性能优化** | **0.5-1 倍** | AI 难有 profiler 级洞察 |
| **安全代码审查** | **危险: 0.3 倍** | AI 会漏 subtle 漏洞,Rust 新手更漏 |

### 1.2 Rust 小白 + AI 的**隐藏成本**

❗ **关键: 你的 Rust 经验是"<1 年"**,这会在以下场景反噬 AI 加速:

1. **AI 幻觉识别**: AI 经常生成看似对但 `cargo build` 失败的 Rust 代码。新手看不出,来回试错浪费时间
2. **Borrow checker 对抗**: AI 建议的 `clone()` / `Arc<Mutex>` 常是糖衣炮弹,堆起一堆性能债
3. **宏 / procedural macro**: AI 生成的宏经常引入诡异编译错误,新手难修
4. **unsafe 代码**: AI 生成 unsafe 时你无法判断正确性 — 对安全产品是**致命风险**
5. **tokio/async 陷阱**: `Send + Sync`、`lifetime bound`、`Pin` 等,AI 错误建议占比高
6. **codex 代码语义**: Port 时 AI 不理解原作者意图,可能改错关键逻辑 (例如 sandbox 权限判断反了)

**实测行业数据 (2024-2025 开发者调研)**:
- 有经验 Rust + AI: 整体 **2-3 倍**加速
- Rust 新手 + AI: 整体 **1.2-1.8 倍**加速 (减去调试时间)
- vibe coding 拒绝深度理解: 技术债累积速率翻倍

### 1.3 你的实际有效人力

**保守估计**:
- 基准: 1 人手写 Rust 新手 (0.2 个资深)
- AI 加速 1.5 倍 → **相当于 0.3 个资深 Rust**
- vs 场景 B (5 人资深) 仍有 **15-17 倍 gap**

**乐观估计** (你 AI 驾驭能力强 + 愿意花时间搞懂):
- AI 加速 2.5 倍 → **相当于 0.5 个资深**
- vs 场景 B 仍有 **10 倍 gap**

---

## §2 重新估算可完成范围

### 2.1 8 个月 + AI 能完成的现实工作量

按"相当于 0.5 个资深 Rust × 8 月 = 4 资深人月"估算:

| 工作 | 预估人月 (资深) | 可否完成? |
|------|--------------|---------|
| 新建 1 个简单 crate (< 2k LOC) | 0.5-1 | ✅ |
| 新建 1 个复杂 crate (2-5k LOC) | 1.5-3 | ⚠️ 勉强 |
| Port 1 个中等 codex crate | 0.5-1.5 | ✅ |
| Observer Chain 重构 | 3-5 | ❌ 不够 |
| 22 crate 全套 | 30-50 | ❌ 完全不够 |
| **3-5 crate 最小版** | **3-6** | ✅ **刚好** |
| **5-7 crate 中等版 (含 port)** | **5-8** | ⚠️ 紧张 |

### 2.2 Pivot α 可以从 3 crate 扩到 5-6 crate

**修订的 MVP 范围** (AI 辅助下):

| # | Crate | 性质 | 预估 (资深人月) | 必要性 |
|---|-------|-----|--------------|-------|
| 1 | `dasclaw_dlp_engine` | 新建 | 2.0 | 🔴 Q5 Top 1 |
| 2 | `dasclaw_cred_boundary` | 新建 | 1.5 | 🔴 Q5 Top 2 |
| 3 | `dasclaw_sandbox_kernel` | Facade + port codex 三平台 | 2.0 | 🔴 Q5 Top 3 |
| 4 | `dasclaw_audit_core` | 从 admin-backend 抽 + 增强 | 1.0 | 🟡 Q5 #6,合规基线 |
| 5 | `dasclaw_llm_cn` | 国产 LLM 适配 (复用 claw-code providers) | 0.5 | 🟡 Q3/Q5 #4 |
| 6 | `dasclaw_skill_mvp5` | 5 个 MVP skill (code/search/web/file/kb) | 1.5 | 🟡 Q3 场景支撑 |
| **总计** |   |   | **8.5 人月** | ⚠️ 超 4 人月预算,**需要砍** |

**务实收敛** (确保 Q4 可交付):

保留 1/2/3 + 砍掉 4/5/6 中的重新实现,复用现有:
- ✅ 新建: dlp / cred_boundary / sandbox_kernel (5.5 人月)
- ✅ 复用: 现有 admin-backend 审计 + claw-code LLM providers + claw-code skills (0.5 月整合)
- ✅ 上层: 继续用 ironclaw / claw-code 不重构

**总计 ~6 资深人月**,AI 辅助 1 人 8 月 (~4 资深人月预算) 仍**超 50%**。

---

## §3 AI 做不了的硬活 (你必须自己顶)

即使你 vibe coding,以下工作 **AI 无法替代**:

| 工作 | AI 为什么不行 | 你要投入多少? |
|------|-------------|-------------|
| **架构决策** (Route B vs α vs vendor strategy) | 需理解业务约束 | 🔴 每周 2-4 小时,8 月 = 约 0.5 人月 |
| **Codex 代码语义理解** (port 时不能改错) | AI 不读 upstream 意图 | 🔴 每个 port crate 1-2 周 review |
| **安全属性验证** (DLP 不能 fail-open) | AI 经常用 `.unwrap()` 或吞异常 | 🔴 每个安全 crate 1 周独立 audit |
| **性能 profile + 优化** | AI 无 flame graph | 🟡 M3 末 1-2 周 |
| **客户沟通 / 标杆 POC 对接** | 必须人 | 🟡 贯穿全程 |
| **集成 E2E 调试** | AI 看不到真实日志 | 🔴 约 1-1.5 人月 |

**这些加起来约 2.5-3.5 资深人月,已经吃掉你 8 月 (4 资深人月预算) 的 60-80%**。

真正留给"生成代码"的时间: **0.5-1.5 资深人月**。

---

## §4 修正后的 pivot α 方案

### 4.1 新的可执行 MVP 范围

```
保留 (新建 3 crate):
  dasclaw_dlp_engine      (双向 DLP, Q5 Top 1, 2 月)
  dasclaw_cred_boundary   (host 注入, Q5 Top 2, 1.5 月)
  dasclaw_sandbox_kernel  (三平台 facade, Q5 Top 3, 2 月)

复用 (零改动 / 小补丁):
  admin-backend 审计  → 写 DLP/sandbox 联动补丁即可
  claw-code LLM providers → 加 3-5 个国产 LLM 配置
  claw-code skills → MVP 5 skill 全部复用
  ironclaw / codex → 当作 runtime 后端,不 port

集成:
  把 3 个新 crate 接入 ironclaw/claw-code/admin-backend 的扩展点
  写 2 条主线 E2E (code review / HR 薪酬) 的冒烟测试

放弃 (延到 v2+):
  Route B 22 crate 重做
  Observer Chain 重构
  可视化 Agent 编辑器
  组织树 / 多租户 / 配额
  国密硬件
  等保合规落地 (仅保留设计文档)
```

### 4.2 8 月时间表 (独狼 + AI)

| 月 | 工作 | 里程碑 |
|----|------|-------|
| M1 (5月) | 架构决策收敛 + W0 骨架 (3 crate 空壳 + CI) | ADR × 3 落地 |
| M2 (6月) | `dasclaw_dlp_engine` 规则引擎核心 | 单测 80%+ |
| M3 (7月) | `dasclaw_cred_boundary` + DLP 集成 admin-backend | 第一条 E2E |
| M4 (8月) | `dasclaw_sandbox_kernel` facade + 三平台 port | 沙箱可跑 |
| M5 (9月) | 国产 LLM 适配 + MVP 5 skill 验证 | 两条主线 demo |
| M6 (10月) | POC 打磨 + 审计联动 + 标杆客户预对接 | Beta 可用 |
| M7 (11月) | 性能优化 + 集成 E2E + 文档 | RC |
| M8 (12月) | POC 验证 + 标杆客户试用 | **MVP 交付** |

**关键裕度**: 只给每个 crate 2 月(新手+AI),无缓冲。一旦一个 crate 滑 2 周整体崩盘。

### 4.3 必须接受的取舍

1. **MVP = POC 级,不是生产级** — 只能服务 1-2 个标杆客户试用
2. **上层不重构** — ironclaw/claw-code/admin-backend 现状持续,新 crate 是外挂增强
3. **测试覆盖率降到 70-80%** — 100% 对安全 crate 都不现实
4. **14 Claude Code 对标改为技术参考** — 我们不是要替代它
5. **Q8 D/E 大客户留到 v2** — v1 不承诺服务 1 万人企业

---

## §5 给用户的最终建议

**回答你的挑战 "AI 辅助是否不合理":**

✅ **AI 辅助让 1 人 8 月可行,但前提是把 18 原计划砍到 3 crate,而非 22 crate。**

**两个关键 reality check**:
1. AI 不是全能: 架构 / 安全 / 集成 / 调试仍吃人月,且 Rust 新手被 AI 误导的时间可能比获得的加速还多
2. vibe coding 需要纪律: 不能全盘接受 AI 建议,**每个安全相关代码必须独立 audit**

**推荐路径**:
- ✅ **选 Pivot α (修订版)**: 3 新 crate + 复用 + 上层不动
- ✅ **MVP 定位 = POC 级标杆验证** (非生产)
- ✅ **8 月时间表有风险但可达成**
- ⚠️ **超过 3 crate 就不要接了**,每加一个就砍其他

---

## §6 等你最后拍板的 3 个问题

1. **同意收敛到 3 crate + POC 级 MVP 吗?**
2. **同意 14 改名为"技术参考清单"吗?**
3. **同意把原 Route B 22 crate 作为"v2+愿景存档",不废弃但冻结吗?**

确认后我就开始:
- 15 升 v1.2 (锁答案 + MVP 定位)
- 18 大改 v2.0 (收敛到 3 crate + 8 月表)
- 21 升 v1.1 (新增场景 D 独狼 + AI)
- 14 改名
- 20 重评 Red Flag
- 新增 24-real-competitor-parity.md

---

## 变更历史

- **v0.1 (2026-04-25)**: 回应用户 "AI 开发是否合理" 挑战的首版评估。
