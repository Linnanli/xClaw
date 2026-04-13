# Plan: 扩展管理增强 — 借鉴 SkillHub 能力

## TL;DR

基于 iflytek/skillhub 的全栈能力分析，增强 x-claw admin-backend 的扩展管理系统。核心升级：异步安全扫描、版本生命周期状态机、审核工作流增强、安全扫描结果 UI。分 5 个阶段递进交付。

**已删除 Phase 4（Token 管理）**：client_token 就是 registered_clients.id（UUID），没有独立字段，客户端管理页已覆盖全部需求。

---

## 新增：受管终端模式（Managed Mode）实施轨道

> 目标：防止 Desktop Client 通过本地复制技能文件或非管理端源安装扩展/技能。

### P0（已完成）：受管模式开关与配置下发

- 管理端在 client-config 下发 managed_mode。
- 客户端启动时注入 MANAGED_MODE 环境变量。
- 管理端可通过系统配置统一启停受管模式。

### P1（已完成）：安装门禁 + 启动收敛

- 技能安装：受管模式下禁止本地 SKILL.md content 直装。
- 扩展安装：受管模式下禁止显式 URL 安装。
- 启动收敛：对本地已安装项目执行授权比对，未授权项自动软禁用。

### P2（已完成）：签名策略清单

- 管理端新增 GET /api/client-policy，返回签名 envelope：algorithm、key_id、manifest_payload、signature。
- 策略内容包含：policy_version、issued_at、expires_at、managed_mode、allowed_skills、allowed_extensions。
- 客户端验签（Ed25519）后执行；支持远端拉取失败时回退本地“已验签缓存”。

### P3（进行中）：密钥轮换 + 防重放 + 可观测性

- 密钥轮换：
  - 管理端支持多 signing key（按 key_id 选择当前签名密钥）。
  - 客户端支持多 verifying key（按 key_id 查找对应公钥验签）。
- 防重放：
  - 客户端持久化 last_policy_version，仅接受版本号单调递增的策略。
  - 对过期策略和旧版本策略拒绝应用，并记录告警日志。
- 可观测性：
  - 启动日志记录 key_id、policy_version、来源（remote/cache）和拒绝原因。
  - 联调脚本覆盖策略下发、验签、版本拒绝路径。

---

## Spec：功能规格说明

### S1. 版本生命周期状态机

当前状态：review_status 只有 pending/approved/rejected，版本是单字段覆盖式更新。

目标状态机：DRAFT → SCANNING → PENDING_REVIEW → PUBLISHED → YANKED / REJECTED → DRAFT

验收标准：
- AC1: skills 和 plugins 表 review_status 扩展值支持 scanning / scan_failed / yanked
- AC2: 已发布技能可被管理员 YANK（下架不删除），前端展示"已下架"状态
- AC3: 被拒绝的技能可重新提交审核（status 回到 pending）
- AC4: 私有注册表 API 只返回 approved + enabled=true 的技能
- AC5: YANKED 技能从注册表搜索结果中排除

### S2. 异步安全扫描

当前状态：同步关键词黑名单（8 个硬编码词），命中直接拒绝上传。

验收标准：
- AC1: 上传成功后 review_status = 'scanning'，后台异步执行扫描
- AC2: 新增 scan_results 表存储扫描结果（verdict/findings/duration）
- AC3: 扫描完成后自动转为 pending（需人工审核）
- AC4: 扫描失败（引擎不可达/超时）标记为 scan_failed，管理员可手动重扫或跳过
- AC5: 管理员审核时可看到扫描结果（verdict + findings 列表）
- AC6: 扫描引擎可配置开关，关闭时直接进入 pending 状态

### S2.1 清单格式与校验兼容（新增）

背景：Claude Skills frontmatter 不要求 `version`，且我们不在 Claude `.claude-plugin/plugin.json` 中增加平台私有字段。

新增平台清单：`manifest.json`（独立于 Claude plugin.json）

```json
{
  "version": "1.0.0",
  "author": "可选，缺省时回退 frontmatter.author"
}
```

说明：`manifest.json` 为可选。当前主要用于补充 `version`（以及可选 `author`），并为后续平台私有元数据扩展预留位置。

验收标准：
- AC1: 上传包支持读取独立 `manifest.json` 作为平台清单（不依赖 plugin.json 承载平台字段）
- AC2: 版本号解析优先级明确且可追溯：
  - `upload-package`：`manifest.json.version` → `SKILL.md frontmatter.version`
  - `upload-skill(json)`：请求体 `version` → `SKILL.md frontmatter.version`
- AC3: 前置格式校验不再强制 `SKILL.md frontmatter.version`；若多源都缺失版本号，入库默认 `1.0.0`
- AC4: 上传缺少版本信息不再返回 400，改为按默认版本落库并继续扫描流程
- AC5: 兼容旧包（仅 frontmatter.version）和新包（frontmatter 无 version，但 manifest.json 有 version）

### S3. 审核工作流增强

验收标准：
- AC1: 审核操作记录真实 reviewer_id（从 Auth header 提取）
- AC2: 前端审核弹窗展示扫描结果摘要（verdict badge + findings 数量）
- AC3: 审核列表按状态分 tab：待审核 / 已通过 / 已拒绝 / 已下架

### S4. 安全扫描结果 UI

验收标准：
- AC1: 技能详情侧 panel 展示安全扫描 section（verdict badge + findings 列表）
- AC2: Verdict 颜色编码：SAFE(绿) / SUSPICIOUS(黄) / DANGEROUS(橙) / BLOCKED(红)
- AC3: Findings 按严重度排序，可折叠展开
- AC4: 每个 finding 显示：规则ID、严重度、标题、文件位置、代码片段、修复建议
- AC5: 扫描中状态显示 loading 指示器

### ~~S5. Token 管理 UI~~（已删除）

client_token 就是 registered_clients.id，不存在独立 token 字段，客户端管理页已完整覆盖。

---

## Design：技术设计

### D1. 数据库变更

新增迁移文件 025_scan_results.sql：

```sql
CREATE TABLE IF NOT EXISTS scan_results (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    target_type VARCHAR(10) NOT NULL,
    target_id UUID NOT NULL,
    scanner_type VARCHAR(50) NOT NULL,
    verdict VARCHAR(20) NOT NULL,
    is_safe BOOLEAN NOT NULL DEFAULT false,
    max_severity VARCHAR(10),
    findings_count INT NOT NULL DEFAULT 0,
    findings JSONB NOT NULL DEFAULT '[]',
    scan_duration_ms INT,
    scanned_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_scan_results_target ON scan_results(target_type, target_id);
```

关键决策：
- target_type + target_id 而非 skill_id：技能和插件共用扫描结果表
- findings 用 JSONB：结构灵活，不同引擎字段不同
- 不新增 skill_versions 表：当前不需要多版本并存

### D2. 后端 Handler 变更

修改文件：admin-backend/src/handlers/extensions.rs

1. upload_skill 重构：前置格式校验 → 异步扫描流程
2. 新增 get_scan_results handler：GET /api/skills/{id}/scan-results
3. 新增 yank_skill / yank_plugin：POST /api/skills/{id}/yank
4. 新增 rescan：POST /api/skills/{id}/rescan
5. review_item 增强：提取真实 reviewer_id
6. 版本解析与校验收敛：入库前统一执行“版本必填（多源解析）”校验

### D3. 后台扫描模块

新增文件：admin-backend/src/scanner.rs

完全移除 INJECTION_KEYWORDS 关键词匹配（cisco-ai-skill-scanner 已覆盖），单一 SkillScanner HTTP 客户端。

行为规则：
- SCANNER_ENABLED=false（默认）：跳过扫描，直接 review_status = 'pending'
- SCANNER_ENABLED=true：调用 POST {SCANNER_URL}/scan-upload
- scanner 不可达/超时 → scan_failed，不降级到关键词检测（Fail-Safe）

配置环境变量：
- SCANNER_ENABLED=false
- SCANNER_URL=http://localhost:8000
- SCANNER_TIMEOUT_MS=30000

### D3.1. Scanner 容器服务

完全复用 iflytek/skillhub 的 scanner/ 目录：

- Dockerfile：pip install cisco-ai-skill-scanner + 启动 skill-scanner-api
- examples/vetter-rules/signatures-append.yaml：9 条 Regex 规则
- examples/vetter-rules/yara/skillhub_vetter.yara：3 条 YARA 规则

新增文件结构：
```
admin-backend/
  scanner/
    Dockerfile
    rules/
      signatures-append.yaml
      yara/
        skillhub_vetter.yara
```

### D4. 前端变更

修改文件：admin-backend/ui/src/pages/extensions.tsx

1. SkillTable 增强：scanning / scan_failed / yanked 状态 badge
2. ReviewModal 增强：内嵌扫描结果 section
3. 新增 ScanResultSection 组件：VerdictBadge + FindingItem
4. 新增 Tab 过滤：全部 / 待扫描 / 待审核 / 已通过 / 已拒绝 / 已下架

---

## Tasks：实施任务清单

### Phase 0：UI 设计（先于后端实施）

**Task 0.1** — Admin Backend 前端 UI 设计（Pencil）
- 文件：docs/backend-design.pen
- 覆盖页面：扩展管理列表（新状态 badge）、审核弹窗（含扫描结果）、Tab 过滤

**Task 0.2** — Desktop Client UI 设计（Pencil）
- 文件：docs/client-design.pen
- 覆盖页面：技能列表（enable/disable Switch）、技能详情 Sheet、扩展管理 Switch

### Phase 1：数据库 + 后端核心（后端先行）

**Task 1.1** — 新增迁移 025_scan_results.sql
- 创建 scan_results 表
- 更新 integration_smoke_tests.rs 的 required 列表
- 验证：cargo test --test integration_smoke_tests

**Task 1.2** — 新增 scanner.rs 扫描模块
- 定义 SecurityVerdict、FindingSeverity、SecurityFinding、ScanResult、ScanError 类型
- 实现 SkillScanner：HTTP 客户端调用 POST {SCANNER_URL}/scan-upload
- 移除 upload_skill 中的 INJECTION_KEYWORDS 和 scan_for_injection()
- 配置读取：SCANNER_ENABLED、SCANNER_URL、SCANNER_TIMEOUT_MS（注入 AppState）
- 同步修复 Bug B1：extensions.rs:499 WHERE rc.client_token → WHERE rc.id
- 测试：scanner_unit_tests.rs（mock HTTP + 成功路径 + 超时失败路径 + 不可达路径）

**Task 1.2b** — 准备 Scanner 容器（复用 SkillHub 资源）
- 复制 skillhub/scanner/Dockerfile → admin-backend/scanner/Dockerfile
- 复制 skillhub/scanner/examples/vetter-rules/ → admin-backend/scanner/rules/
- 更新 admin-backend/docker-compose.yml，新增 skill-scanner 服务（含 vetter-rules 卷挂载）
- 验证：curl http://localhost:8000/health 返回 200

**Task 1.3** — 重构 upload_skill 为异步扫描流程
- 前置：SKILL.md 格式校验（安全扫描之前执行）：
  - 解析 YAML frontmatter（serde_yaml 或手动切割 --- 块）
  - 校验必填字段：name（字母数字开头，允许点/下划线/连字符，最长 64 字符）、description（非空）
  - 其他 frontmatter 字段（如 activation/keywords/patterns/tags）均为非必填；若提供则按各自格式规则校验
  - 版本号改为“多源回退 + 默认值”规则：
    - upload-package：`manifest.json.version` 优先，回退 `frontmatter.version`
    - upload-skill(json)：请求体 `version` 优先，回退 `frontmatter.version`
    - 两者都缺失时使用默认值 `1.0.0`
  - 校验限制：keywords/exclude_keywords 最多 20 个且每个至少 3 字符；patterns 最多 5 个；tags 最多 10 个；文件最大 64 KiB
  - 校验失败 → 400 Validation 错误，不进入扫描流程
- 上传成功 → review_status = 'scanning'
- tokio::spawn 触发扫描 → 写入 scan_results → 更新 review_status = 'pending'
- 扫描失败 → review_status = 'scan_failed'
- 测试：extensions_unit_tests.rs 更新（格式校验失败路径 + 清单版本回退路径 + 扫描流程路径）

**Task 1.4** — 新增扫描结果查询 API
- GET /api/skills/{id}/scan-results
- GET /api/plugins/{id}/scan-results
- 路由注册到 routes.rs
- 测试：契约测试

**Task 1.5** — 新增 yank API
- POST /api/skills/{id}/yank
- POST /api/plugins/{id}/yank
- 路由注册
- 测试：单元 + 失败路径

**Task 1.6** — 新增 rescan API
- POST /api/skills/{id}/rescan
- 验证：只有 scan_failed 状态可重扫
- 测试：单元 + 失败路径

**Task 1.7** — 审核增强：reviewer_id
- review_item 函数提取 reviewer_id（暂用 header X-Admin-User-Id）
- 写入 reviewed_by 字段（迁移 023 已有该字段）
- 测试：验证审计日志包含 reviewer_id

### Phase 2：前端扫描结果 UI

**Task 2.1** — ScanResultSection 组件
- VerdictBadge 组件（颜色映射）
- SeverityBadge 组件
- FindingItem 组件（展开/折叠）
- 从 GET /api/skills/{id}/scan-results 获取数据

**Task 2.2** — ReviewModal 集成扫描结果
- 审核弹窗内嵌 ScanResultSection

**Task 2.3** — 状态列增强
- 新增 scanning、scan_failed、yanked 状态 badge

**Task 2.4** — 操作按钮
- "下架" 按钮（POST /api/skills/{id}/yank）
- "重新扫描" 按钮（POST /api/skills/{id}/rescan）

### Phase 3：审核列表 + 过滤

**Task 3.1** — 审核列表 Tab 过滤
- 全部 / 待扫描 / 待审核 / 已通过 / 已拒绝 / 已下架
- 各 tab 数量徽标

**Task 3.2** — 审核历史展示
- 显示 reviewer 信息（reviewed_by + reviewed_at）
- 审核备注展开

### ~~Phase 4：Token 管理~~（已删除）

client_token 就是 registered_clients.id，无需独立模块。

### Phase 5：桌面客户端 — 技能/插件管理联动

当前链路状态：

| 功能点 | 链路状态 |
|--------|---------|
| 技能审核通过/启用 | 完整 |
| 技能下架（YANK） | 断裂：注册表搜索未过滤 yanked |
| 部门白名单过滤 | 完整（修复 Bug B1 后） |
| 扩展启用/禁用 | 断裂：前端 Switch 无实现 |
| 技能启用/禁用 | 断裂：无 UI 也无后端命令 |

**Task 5.1** — 注册表搜索过滤 yanked 状态
- registry_search 和 fetch_all_enabled_skills 增加 AND review_status != 'yanked' 条件
- 测试：单元测试确认 yanked 技能不出现在 /api/v1/search 结果中

**Task 5.2** — 客户端扩展启用/禁用 Tauri 命令
- 新增 ic_enable_extension / ic_disable_extension
- 注册到 all_tauri_commands!() + 更新 tauri_command_contract_tests.rs

**Task 5.3** — 客户端技能启用/禁用 Tauri 命令
- 新增 ic_enable_skill / ic_disable_skill
- SkillsTab.tsx 为非内置技能增加 Switch 开关
- 注册命令 + 更新契约测试

**Task 5.4** — SkillsTab 技能详情 Sheet
- 实现"查看详情"：name、version、description、keywords、source
- 无需新 Tauri 命令（数据已在 SkillInfo 中）

测试要求：

| 测试类型 | 文件 | 覆盖内容 |
|---------|------|---------|
| 契约测试 | tests/tauri_command_contract_tests.rs | ic_enable_extension/disable/skill 注册验证 |
| 单元测试 | src/ipc/extensions_tests.rs | enable/disable 成功+失败路径 |
| 启动时序 | src/engine_startup_tests.rs | ExtensionManager 状态不受影响 |

---

## 验证步骤

1. cargo build -p admin-backend — 0 错误 0 警告
2. cargo test -p admin-backend — 所有测试通过
3. cargo test --test integration_smoke_tests — 迁移完整性验证
4. 前端 TypeScript 编译无错误
5. 手动测试：上传技能 → scanning 状态 → 自动转 pending → 审核时看到扫描结果

---

## 运行时 Bug（需在实施阶段修复）

**Bug B1 — extensions.rs:499 查询不存在的列**

- 文件：admin-backend/src/handlers/extensions.rs L499
- 现象：resolve_department_id() 使用 WHERE rc.client_token = $1，但 registered_clients 表（migration 009）没有 client_token 列，运行时报错
- 影响：部门白名单过滤完全失效，/api/v1/search 和 /api/v1/download 对所有客户端返回完整列表
- 修复：WHERE rc.client_token = $1 改为 WHERE rc.id = $1
- 合并到：Task 1.2 一并修复

---

## 背景参考：ironclaw Skills 沙箱能力边界

| 能力 | 现状 | 文件 |
|------|------|------|
| 网络白名单 | 全局代理过滤（crates.io、pypi.org、github.com 等） | ironclaw/src/sandbox/proxy/allowlist.rs |
| 命令控制 | 全局三层过滤（BLOCKED_COMMANDS + DANGEROUS_PATTERNS） | ironclaw/src/tools/builtin/shell.rs |
| Installed skills 工具限制 | 信任级别衰减，限制为 8 个只读工具 | ironclaw/src/skills/attenuation.rs |
| Per-skill 网络声明 | 不存在（SKILL.md 无 allowed_hosts 字段） | — |
| Per-skill 命令声明 | 不存在（SKILL.md 无 allowed_commands 字段） | — |

vetter-rules 中检测外发请求（curl/wget、IP 直连）弥补了 per-skill 声明不存在的空白。

---

## 决策记录

- 不引入 skill_versions 表：当前不需要多版本并存
- 不引入 Redis Stream：扫描量小，tokio::spawn 足够
- 不引入 Namespace：x-claw 用"部门"概念替代，已有 department_skill_whitelist
- scan_results 表独立于 skills：方便扩展到 plugins 等实体
- reviewer_id 暂用 X-Admin-User-Id header：等 auth middleware 完善后改为自动提取

---

## 范围排除

- 评分/收藏社交功能（P3）
- 标签系统（需确认业务需求）
- CLI 发布流程（x-claw 走 admin 上传）
- 推广到全局空间（无全局空间概念）

---

## TODO：Per-Skill 运行时沙箱声明（未来方向）

当前 ironclaw 的网络白名单和命令过滤均为全局配置，SKILL.md 里没有 per-skill 级别的声明字段。

拟扩展的 SKILL.md schema：

```yaml
metadata:
  openclaw:
    requires:
      bins: [curl]
      env: [API_KEY]
    sandbox:
      allowed_hosts:
        - "api.github.com"
        - "*.openai.com"
      allowed_commands:
        - "git"
        - "cargo"
```

需要修改的 ironclaw 文件：

| 文件 | 改动 |
|------|------|
| ironclaw/src/skills/mod.rs | GatingRequirements 新增 sandbox 字段 |
| ironclaw/src/skills/parser.rs | YAML 解析支持新字段 |
| ironclaw/src/sandbox/proxy/allowlist.rs | 支持 per-skill 覆盖（取交集，不能超出全局） |
| ironclaw/src/tools/builtin/shell.rs | 执行前检查当前 Skill 上下文的 allowed_commands |
| ironclaw/src/skills/attenuation.rs | 多技能激活时取最严格策略（交集） |

安全原则：per-skill 声明只能收窄全局白名单，不能扩展；多技能同时激活取交集（最小权限）。

优先级：低（当前静态扫描方案已覆盖大部分威胁）
