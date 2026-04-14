# 项目级 PICT 测试设计

生成时间：2026-04-14

## 目标

本文档把当前仓库的核心测试面压缩为 3 个项目级 PICT 子模型，用 pairwise 方式生成一组高密度但可维护的测试组合，覆盖以下高风险区域：

- Admin Backend 的权限、校验、迁移、DLP、扩展、模型配置
- Desktop Client 的 DLP 扫描、策略同步、启动时序、审批 UI、日志
- 端到端跨层边界的 HTTP、Tauri IPC、SSE、迁移、受管策略

这份设计不替代模块级 TDD，也不替代现有失败路径测试、契约测试和安全审计测试。它的作用是为项目级回归和新增测试提供统一矩阵，避免只测成功路径或只测单层 mock。

## 设计依据

本设计基于以下仓库事实整理：

- 管理后台测试已经按单元、失败路径、契约、可靠性、安全审计和冒烟拆分，见 [admin-backend/tests](admin-backend/tests)
- 桌面端已经存在鉴权、策略同步、模型白名单、Tauri 命令契约等测试，见 [desktop-client/tests](desktop-client/tests)
- 项目明确要求失败路径、安全审计、真实环境和契约测试同等重要，见 [AGENTS.md](AGENTS.md) 和 [docs/testing-guide.md](docs/testing-guide.md)
- 已知历史盲区集中在 DLP Fail-Open、SSE 事件名不匹配、迁移缺失、字段不足、Tauri 状态未注入，见 [TESTING_GUIDE.md](TESTING_GUIDE.md)
- 当前规划里还强调了受管策略、扫描状态机和跨层协同风险，见 [plan.md](plan.md)

## 使用方式

1. 先按本文档选择对应子模型，而不是一次性实现全部 80 条。
2. 每个子模型的 pairwise 用例作为基础集执行。
3. 每个子模型后面的种子用例必须保留，不能因为 pairwise 已覆盖而删除。
4. 落地到代码时，继续遵守现有测试分层：单元、失败路径、契约、安全审计、集成、E2E。

## 总览

| 子模型 | 目标 | Pairwise 用例 | 强制种子 |
| --- | --- | ---: | ---: |
| M1 | Admin Backend 管理平面 | 25 | 1 |
| M2 | Desktop Client 执行平面 | 24 | 3 |
| M3 | 端到端跨层边界 | 26 | 1 |
| 合计 | 项目级测试矩阵 | 75 | 5 |

总计：80 条。

---

## M1: Admin Backend 管理平面

### 适用范围

- Auth
- Department
- DLP
- Extensions
- ModelConfig

### PICT Model

```text
Environment: LocalUnit, DockerIntegration, CI
Domain: Auth, Department, DLP, Extensions, ModelConfig
AuthState: Anonymous, UserToken, AdminToken
FailureMode: Success, ValidationError, DbError, Unauthorized
VerificationLayer: Unit, Integration, Contract, SecurityAudit, Smoke

IF [VerificationLayer] = "Smoke" THEN [Environment] <> "LocalUnit";
```

### 预期结果字典

| 代码 | 预期结果 |
| --- | --- |
| A1 | 鉴权成功或登录成功，响应结构正确 |
| A2 | 返回 400，校验错误可诊断，且无敏感数据泄露 |
| A3 | 返回 5xx 或错误对象，错误信息和日志不泄露敏感数据 |
| A4 | 返回 401 或 403，无副作用，权限边界正确 |
| A5 | 部门配置成功落库，并能被下游读取 |
| A6 | DLP 规则成功写入并生效，失败时默认拒绝 |
| A7 | 扩展或技能状态更新成功，注册表仅暴露 approved 且 enabled 的项 |
| A8 | 模型配置字段完整，足以支持客户端实际发起 LLM 调用 |

### 生成测试用例

| # | Environment | Domain | AuthState | FailureMode | VerificationLayer | Expected |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | LocalUnit | Auth | Anonymous | Success | Unit | A1 |
| 2 | LocalUnit | Department | UserToken | ValidationError | Integration | A2 |
| 3 | LocalUnit | DLP | AdminToken | DbError | Contract | A3 |
| 4 | DockerIntegration | Auth | UserToken | DbError | SecurityAudit | A3 |
| 5 | DockerIntegration | Department | Anonymous | Unauthorized | Contract | A4 |
| 6 | DockerIntegration | Extensions | AdminToken | Success | Integration | A7 |
| 7 | CI | Auth | AdminToken | ValidationError | Smoke | A2 |
| 8 | CI | DLP | UserToken | Unauthorized | Unit | A4 |
| 9 | CI | ModelConfig | Anonymous | DbError | Integration | A3 |
| 10 | LocalUnit | ModelConfig | AdminToken | Unauthorized | SecurityAudit | A4 |
| 11 | DockerIntegration | DLP | Anonymous | ValidationError | SecurityAudit | A2 |
| 12 | DockerIntegration | ModelConfig | UserToken | Success | Smoke | A8 |
| 13 | CI | Department | AdminToken | Success | SecurityAudit | A5 |
| 14 | CI | Extensions | UserToken | ValidationError | Contract | A2 |
| 15 | LocalUnit | Extensions | Anonymous | DbError | Unit | A3 |
| 16 | DockerIntegration | ModelConfig | AdminToken | ValidationError | Unit | A2 |
| 17 | DockerIntegration | Department | Anonymous | DbError | Smoke | A3 |
| 18 | LocalUnit | Auth | Anonymous | Unauthorized | Integration | A4 |
| 19 | DockerIntegration | Extensions | Anonymous | Unauthorized | Smoke | A4 |
| 20 | LocalUnit | Auth | Anonymous | Success | Contract | A1 |
| 21 | LocalUnit | DLP | Anonymous | Success | Integration | A4 |
| 22 | LocalUnit | Department | Anonymous | Success | Unit | A4 |
| 23 | LocalUnit | Extensions | Anonymous | Success | SecurityAudit | A4 |
| 24 | LocalUnit | ModelConfig | Anonymous | Success | Contract | A4 |
| 25 | DockerIntegration | DLP | Anonymous | Success | Smoke | A4 |

### 强制种子用例

这些用例不是 pairwise 自动补出来的，但必须保留：

| # | Environment | Domain | AuthState | FailureMode | VerificationLayer | Expected |
| --- | --- | --- | --- | --- | --- | --- |
| M1-S1 | DockerIntegration | DLP | AdminToken | Success | Integration | A6 |

### 落地建议

- Auth 优先落在现有 auth 相关单元和契约测试中
- Department、DLP、Extensions、ModelConfig 优先追加到各自 failure、contract、security audit 和 integration 测试文件
- CI + Smoke 组合优先映射到 [admin-backend/tests/integration_smoke_tests.rs](admin-backend/tests/integration_smoke_tests.rs)

---

## M2: Desktop Client 执行平面

### 适用范围

- DlpScan
- PolicySync
- Startup
- ApprovalUI
- Logs

### PICT Model

```text
Runtime: UnitHarness, RealTauri
FeatureArea: DlpScan, PolicySync, Startup, ApprovalUI, Logs
SessionState: LoggedOut, TokenExpired, TokenValid
Fault: Success, NetworkTimeout, PolicySignatureInvalid, StateNotManaged
VerificationLayer: Unit, Integration, Contract, E2E

IF [VerificationLayer] = "E2E" THEN [Runtime] = "RealTauri";
IF [Fault] = "PolicySignatureInvalid" THEN [FeatureArea] = "PolicySync";
IF [Fault] = "StateNotManaged" THEN [FeatureArea] = "Startup";
```

### 预期结果字典

| 代码 | 预期结果 |
| --- | --- |
| B1 | DLP 正常扫描或脱敏成功，无敏感内容泄露 |
| B2 | DLP 遇到网络异常时 Fail-Safe 阻断发送，不回传原文 |
| B3 | 策略拉取、验签、版本比较全部成功 |
| B4 | 签名无效策略被拒绝，沿用已验签缓存并记录拒绝原因 |
| B5 | 启动状态注入完整，无 panic，状态机转换正确 |
| B6 | 状态未托管时给出友好失败，不 panic |
| B7 | 工具审批卡片的批准和拒绝链路工作正常 |
| B8 | 日志真实显示，搜索、过滤、导出、清空流程正常 |
| B9 | 网络异常进入可恢复状态，允许重试或提示离线，不静默成功 |
| B10 | 登录态无效时要求重新登录或刷新 token，不把受保护能力当成成功处理 |

### 生成测试用例

| # | Runtime | FeatureArea | SessionState | Fault | VerificationLayer | Expected |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | UnitHarness | DlpScan | LoggedOut | Success | Unit | B10 |
| 2 | UnitHarness | PolicySync | TokenExpired | NetworkTimeout | Integration | B10 |
| 3 | UnitHarness | Startup | TokenValid | StateNotManaged | Contract | B6 |
| 4 | RealTauri | DlpScan | TokenValid | NetworkTimeout | E2E | B2 |
| 5 | RealTauri | PolicySync | LoggedOut | PolicySignatureInvalid | Contract | B4 |
| 6 | RealTauri | Startup | TokenExpired | Success | Unit | B10 |
| 7 | RealTauri | ApprovalUI | TokenValid | Success | Integration | B7 |
| 8 | UnitHarness | PolicySync | TokenValid | PolicySignatureInvalid | Unit | B4 |
| 9 | UnitHarness | ApprovalUI | LoggedOut | NetworkTimeout | Unit | B10 |
| 10 | UnitHarness | Logs | TokenExpired | Success | Contract | B8 |
| 11 | RealTauri | Startup | LoggedOut | StateNotManaged | Integration | B6 |
| 12 | RealTauri | Logs | LoggedOut | Success | E2E | B8 |
| 13 | RealTauri | PolicySync | TokenExpired | PolicySignatureInvalid | E2E | B4 |
| 14 | UnitHarness | DlpScan | TokenExpired | NetworkTimeout | Contract | B2 |
| 15 | UnitHarness | Logs | TokenValid | NetworkTimeout | Unit | B9 |
| 16 | RealTauri | Startup | TokenExpired | StateNotManaged | E2E | B6 |
| 17 | UnitHarness | ApprovalUI | TokenExpired | Success | Contract | B10 |
| 18 | UnitHarness | DlpScan | LoggedOut | Success | Integration | B10 |
| 19 | UnitHarness | PolicySync | LoggedOut | Success | Unit | B10 |
| 20 | UnitHarness | PolicySync | LoggedOut | PolicySignatureInvalid | Integration | B4 |
| 21 | UnitHarness | Startup | LoggedOut | NetworkTimeout | Unit | B9 |
| 22 | UnitHarness | Startup | LoggedOut | StateNotManaged | Unit | B6 |
| 23 | UnitHarness | Logs | LoggedOut | Success | Integration | B8 |
| 24 | RealTauri | ApprovalUI | LoggedOut | Success | E2E | B10 |

### 强制种子用例

| # | Runtime | FeatureArea | SessionState | Fault | VerificationLayer | Expected |
| --- | --- | --- | --- | --- | --- | --- |
| M2-S1 | RealTauri | DlpScan | TokenValid | Success | E2E | B1 |
| M2-S2 | RealTauri | PolicySync | TokenValid | Success | Integration | B3 |
| M2-S3 | RealTauri | Startup | TokenValid | Success | E2E | B5 |

### 落地建议

- Startup 相关用例优先映射到 [desktop-client/src/engine_startup_tests.rs](desktop-client/src/engine_startup_tests.rs)
- Tauri IPC 和命令完整性继续由 [desktop-client/tests/tauri_command_contract_tests.rs](desktop-client/tests/tauri_command_contract_tests.rs) 承接
- PolicySync、Managed Skills 和 Model Whitelist 相关失败路径可优先复用 [desktop-client/tests/managed_skills_sync_tests.rs](desktop-client/tests/managed_skills_sync_tests.rs) 和 [desktop-client/tests/policy_sync_property_tests.rs](desktop-client/tests/policy_sync_property_tests.rs)

---

## M3: 端到端跨层边界

### 适用范围

- HTTPRoute
- TauriIPC
- SSEStream
- Migration
- ManagedPolicy

### PICT Model

```text
Surface: HTTPRoute, TauriIPC, SSEStream, Migration, ManagedPolicy
Environment: DockerDB, BrowserE2E, RealTauri
BackendState: Healthy, MissingMigration, SchemaMismatch, EventFormatMismatch, PolicyReplay
DataShape: Complete, MissingField, SensitivePayload
AssertionType: HappyPath, Contract, SecurityAudit, Recovery

IF [Surface] = "Migration" THEN [Environment] = "DockerDB";
IF [Surface] = "SSEStream" THEN [Environment] = "BrowserE2E";
IF [Surface] IN {"TauriIPC", "ManagedPolicy"} THEN [Environment] = "RealTauri";
IF [BackendState] = "MissingMigration" THEN [Surface] = "Migration";
IF [BackendState] = "EventFormatMismatch" THEN [Surface] = "SSEStream";
IF [BackendState] = "PolicyReplay" THEN [Surface] = "ManagedPolicy";
```

### 预期结果字典

| 代码 | 预期结果 |
| --- | --- |
| C1 | 正常链路成功，状态码、事件、IPC 返回值和 UI 结果一致 |
| C2 | 契约测试明确失败，指出缺字段或 schema 不兼容，而不是静默通过 |
| C3 | 安全审计通过，敏感内容不出现在日志、错误、网络回包或 UI 明文中 |
| C4 | 恢复路径成立，系统能回退、重试或重连，而不是卡死 |
| C5 | 迁移缺失时冒烟测试快速失败，避免出现 cargo test 通过但运行 404 |
| C6 | SSE 事件格式不匹配被真实 E2E 抓到，前端不得静默成功 |
| C7 | 重放或旧版本策略被拒绝，本地已验签缓存继续生效 |

### 生成测试用例

| # | Surface | Environment | BackendState | DataShape | AssertionType | Expected |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | HTTPRoute | DockerDB | Healthy | Complete | HappyPath | C1 |
| 2 | HTTPRoute | BrowserE2E | SchemaMismatch | MissingField | Contract | C2 |
| 3 | TauriIPC | RealTauri | Healthy | MissingField | SecurityAudit | C3 |
| 4 | SSEStream | BrowserE2E | Healthy | SensitivePayload | Recovery | C4 |
| 5 | Migration | DockerDB | MissingMigration | MissingField | Recovery | C5 |
| 6 | ManagedPolicy | RealTauri | SchemaMismatch | Complete | Recovery | C4 |
| 7 | SSEStream | BrowserE2E | EventFormatMismatch | Complete | SecurityAudit | C6 |
| 8 | Migration | DockerDB | SchemaMismatch | SensitivePayload | SecurityAudit | C3 |
| 9 | ManagedPolicy | RealTauri | PolicyReplay | SensitivePayload | HappyPath | C7 |
| 10 | SSEStream | BrowserE2E | SchemaMismatch | MissingField | HappyPath | C2 |
| 11 | Migration | DockerDB | Healthy | Complete | Contract | C1 |
| 12 | TauriIPC | RealTauri | SchemaMismatch | SensitivePayload | Contract | C2 |
| 13 | ManagedPolicy | RealTauri | PolicyReplay | MissingField | Contract | C7 |
| 14 | HTTPRoute | RealTauri | Healthy | SensitivePayload | SecurityAudit | C3 |
| 15 | SSEStream | BrowserE2E | EventFormatMismatch | MissingField | Contract | C6 |
| 16 | Migration | DockerDB | MissingMigration | Complete | HappyPath | C5 |
| 17 | ManagedPolicy | RealTauri | PolicyReplay | Complete | SecurityAudit | C7 |
| 18 | TauriIPC | RealTauri | Healthy | Complete | HappyPath | C1 |
| 19 | SSEStream | BrowserE2E | EventFormatMismatch | SensitivePayload | HappyPath | C6 |
| 20 | Migration | DockerDB | MissingMigration | SensitivePayload | Contract | C5 |
| 21 | HTTPRoute | DockerDB | Healthy | Complete | Recovery | C4 |
| 22 | TauriIPC | RealTauri | Healthy | Complete | Recovery | C4 |
| 23 | SSEStream | BrowserE2E | EventFormatMismatch | Complete | Recovery | C6 |
| 24 | Migration | DockerDB | MissingMigration | Complete | SecurityAudit | C5 |
| 25 | ManagedPolicy | RealTauri | Healthy | Complete | HappyPath | C1 |
| 26 | ManagedPolicy | RealTauri | PolicyReplay | Complete | Recovery | C7 |

### 强制种子用例

| # | Surface | Environment | BackendState | DataShape | AssertionType | Expected |
| --- | --- | --- | --- | --- | --- | --- |
| M3-S1 | SSEStream | BrowserE2E | Healthy | Complete | HappyPath | C1 |

### 落地建议

- Migration 缺失相关组合统一映射到 [admin-backend/tests/integration_smoke_tests.rs](admin-backend/tests/integration_smoke_tests.rs)
- SSE 事件名和流式协议组合优先映射到浏览器 E2E
- ManagedPolicy 和 TauriIPC 组合优先映射到真实 Tauri 运行环境，不要只做纯函数单测

---

## 高优先级执行顺序

如果只准备先落地一部分，建议按下面顺序：

1. M1-S1, M2-S1, M2-S2, M2-S3, M3-S1 这 5 条种子用例
2. M3 中所有 C5、C6、C7 用例
3. M1 中所有 A6、A8 用例
4. M2 中所有 B2、B4、B5、B6 用例

理由很简单：这些组合直接对应仓库里已经出现过或明确记录过的真实缺陷类型。

## 注意事项

- 对安全功能不能接受 Fail-Open 测试预期
- 对 SSE、Tauri 状态注入、受管策略验签这类问题，mock 结果不能替代真实环境验证
- 对模型配置、客户端配置、策略下发等接口，契约测试必须验证“够用”，而不是只验证“能解析”
- 每次新增迁移仍然必须同步更新 [admin-backend/tests/integration_smoke_tests.rs](admin-backend/tests/integration_smoke_tests.rs) 的 required 列表
