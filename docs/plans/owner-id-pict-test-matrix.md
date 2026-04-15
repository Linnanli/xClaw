# Owner Identity Split PICT Test Matrix

本文件用于验证 x-claw 在 owner_id 语义拆分后的关键行为：

- `scope_id` 继续代表 IronClaw 本地租户/持久化隔离键
- `backend_user_id` / `backend_principal_id` 单独代表 Admin Backend 真实用户身份
- 审批、配额、模型过滤、对话审计不再复用 `owner_id` 猜后台 UUID

说明：当前工作区未安装 `pict` 或 `pypict`，因此本文给出可直接喂给 PICT 的模型，以及按 pairwise 思路手工整理的测试矩阵。

依据实现：

- [desktop-client/src/state.rs](desktop-client/src/state.rs)
- [desktop-client/src/admin_sync.rs](desktop-client/src/admin_sync.rs)
- [desktop-client/src/conversation_tracker.rs](desktop-client/src/conversation_tracker.rs)
- [desktop-client/src/ipc/chat.rs](desktop-client/src/ipc/chat.rs)
- [desktop-client/src/ipc/models.rs](desktop-client/src/ipc/models.rs)
- [desktop-client/src/commands.rs](desktop-client/src/commands.rs)
- [admin-backend/src/routes.rs](admin-backend/src/routes.rs)

## Model 1

目标：验证 `backend_principal_id -> backend_user_id` 的建立、缓存恢复、缺失降级是否正确。

### PICT Model

```text
# Client token and sync path
ClientTokenKind: UuidClientToken, NonUuidToken, EmptyToken
IdentitySource: FreshClientConfig, CacheFallback, SyncDisabled
BackendPrincipalField: ValidUuid, Missing
ObservationPoint: StartupInit, RuntimeRefresh

# Constraints
IF [ClientTokenKind] = "EmptyToken" THEN [IdentitySource] = "SyncDisabled";
IF [IdentitySource] = "SyncDisabled" THEN [BackendPrincipalField] = "Missing";
IF [IdentitySource] = "FreshClientConfig" THEN [ClientTokenKind] <> "EmptyToken";
IF [IdentitySource] = "CacheFallback" THEN [ObservationPoint] = "StartupInit";
```

### Generated Test Cases

| Test # | ClientTokenKind | IdentitySource | BackendPrincipalField | ObservationPoint | Expected Output |
| --- | --- | --- | --- | --- | --- |
| 1 | UuidClientToken | FreshClientConfig | ValidUuid | StartupInit | `AdminConfigSync::fetch_once` 成功，`backend_user_id` 在启动期被填充，后续审批/配额/模型过滤可直接使用真实后台 UUID。 |
| 2 | UuidClientToken | FreshClientConfig | Missing | StartupInit | 启动期同步成功但未返回 `backend_principal_id`，`backend_user_id` 保持 `None`，后续敏感操作进入缺失身份分支。 |
| 3 | NonUuidToken | FreshClientConfig | ValidUuid | StartupInit | 即使 token 不是 UUID，只要 `/api/client-config` 返回合法 `backend_principal_id`，桌面端仍应建立后台身份，不再依赖 owner_id/JWT 猜测。 |
| 4 | NonUuidToken | FreshClientConfig | Missing | RuntimeRefresh | 运行期同步刷新后仍缺失后台身份，`backend_user_id` 应保持 `None`，不能残留旧值。 |
| 5 | UuidClientToken | CacheFallback | ValidUuid | StartupInit | 首次远端拉取失败，但本地缓存含合法 `backend_principal_id`，启动时应恢复该 UUID 并供会话后续使用。 |
| 6 | UuidClientToken | CacheFallback | Missing | StartupInit | 远端失败且缓存无后台身份，`backend_user_id` 应保持 `None`，不能伪造或沿用 `scope_id`。 |
| 7 | EmptyToken | SyncDisabled | Missing | StartupInit | 未配置 `ADMIN_AUTH_TOKEN`，不启动配置同步，`backend_user_id` 为 `None`，系统仅保留本地 `scope_id` 语义。 |
| 8 | UuidClientToken | FreshClientConfig | ValidUuid | RuntimeRefresh | 运行期配置刷新返回新的合法 UUID 时，共享 sink 应更新 `AppState` 与 `ConversationTracker` 读取到的后台身份。 |

### Test Case Summary

- Total test cases: 8
- Coverage: Pairwise
- Constraints applied: 4

## Model 2

目标：验证建立后的后台身份如何影响各类敏感路径，包括 fail-safe、降级和只读过滤行为。

### PICT Model

```text
# Consumer behavior after identity split
Operation: ConversationAudit, QuotaPrecheck, UsageReport, ModelFetch, ApprovalSubmit
BackendUserState: ReadyUuid, Missing, ReadFailure
AdminRouteState: Success, Denied, Unavailable, NotApplicable

# Constraints
IF [Operation] = "ConversationAudit" THEN [AdminRouteState] = "NotApplicable";
IF [Operation] = "UsageReport" THEN [AdminRouteState] <> "Denied";
IF [Operation] = "UsageReport" THEN [AdminRouteState] <> "NotApplicable";
IF [Operation] = "ModelFetch" THEN [AdminRouteState] <> "Denied";
IF [Operation] = "ModelFetch" THEN [AdminRouteState] <> "NotApplicable";
IF [Operation] = "ApprovalSubmit" THEN [AdminRouteState] <> "Denied";
IF [Operation] = "ApprovalSubmit" THEN [AdminRouteState] <> "NotApplicable";
IF [Operation] = "QuotaPrecheck" THEN [AdminRouteState] <> "NotApplicable";
```

### Generated Test Cases

| Test # | Operation | BackendUserState | AdminRouteState | Expected Output |
| --- | --- | --- | --- | --- |
| 1 | ConversationAudit | ReadyUuid | NotApplicable | 上报时 `ClientReport::Conversation.user_id` 使用后台 UUID；`client_conversation_id` 仍保留 `scope_id-thread-report_id` 形式，区分本地审计分段与后台用户身份。 |
| 2 | ConversationAudit | Missing | NotApplicable | 上报时 `user_id` 回退为 `scope_id`，由后端 `/api/client-reports` 基于 bearer client token 回填 `registered_clients.user_id`。 |
| 3 | ConversationAudit | ReadFailure | NotApplicable | 读取共享身份锁失败时也应回退为 `scope_id`，不能 panic，保证对话审计链路继续可用。 |
| 4 | QuotaPrecheck | ReadyUuid | Success | `quota_precheck()` 发送真实后台 UUID 到 `/api/quota/check`，后端返回 `allowed=true` 时继续聊天流程。 |
| 5 | QuotaPrecheck | ReadyUuid | Denied | `/api/quota/check` 返回 `allowed=false` 时，前端应 fail-safe 拒绝请求，并展示服务端 reason。 |
| 6 | QuotaPrecheck | ReadyUuid | Unavailable | 配额服务超时/5xx 时，前端应 fail-safe 拒绝，不允许在身份已知但配额未知时继续调用模型。 |
| 7 | QuotaPrecheck | Missing | Success | 即使后台服务可用，只要 `backend_user_id` 缺失，也必须在本地直接报错“后台用户身份尚未就绪”，不得退回 `scope_id`。 |
| 8 | QuotaPrecheck | ReadFailure | Success | 共享身份锁读取失败时，应返回“后台用户身份读取失败”而不是降级放行。 |
| 9 | UsageReport | ReadyUuid | Success | `/api/quota/report-usage` 使用真实后台 UUID 上报 token 消耗，后端可正确写入 `usage_records`。 |
| 10 | UsageReport | Missing | Success | 缺失后台身份时，费用上报应记录 warning 并跳过，不阻塞主聊天流程，也不发送错误 `scope_id`。 |
| 11 | UsageReport | ReadFailure | Success | 共享身份锁读取失败时也应仅 warning + skip，不产生错误 payload。 |
| 12 | UsageReport | ReadyUuid | Unavailable | 上报失败只记 warning，不影响主流程，验证“观察性失败不阻塞业务”。 |
| 13 | ModelFetch | ReadyUuid | Success | `fetch_admin_models()` 应请求 `/api/client-models?user_id=<backend_uuid>`，模型列表受部门白名单过滤。 |
| 14 | ModelFetch | Missing | Success | 后台身份缺失时，应请求 `/api/client-models` 无 `user_id` 参数版本，回退为通用模型列表。 |
| 15 | ModelFetch | ReadFailure | Unavailable | 共享身份锁读取失败时，前端应返回读取错误或请求失败，不应 silently 注入错误身份。 |
| 16 | ApprovalSubmit | ReadyUuid | Success | `resolve_applicant_id()` 使用 `backend_user_id` 构造审批申请人，提交审批单成功。 |
| 17 | ApprovalSubmit | Missing | Success | 后台身份缺失时，本地立即报错“后台用户身份尚未就绪，无法提交审批工单”，不得再走 owner_id/JWT 推断。 |
| 18 | ApprovalSubmit | ReadFailure | Unavailable | 身份读取失败时立即返回配置错误；即使后端不可用，也不应先用错误身份发起请求。 |

### Test Case Summary

- Total test cases: 18
- Coverage: Pairwise
- Constraints applied: 5

## Recommended Mapping

建议把上述矩阵拆到现有测试族，而不是另起一套“大而全”测试：

1. `desktop-client/src/admin_sync_tests.rs`
   追加 Model 1 的身份建立/缓存恢复/空 token 降级测试。
2. `desktop-client/src/conversation_tracker.rs`
   已有 `test_report_uses_backend_user_id_when_available`，继续补 Missing 与 ReadFailure 分支。
3. `desktop-client/src/ipc/chat_tests.rs`
   追加 QuotaPrecheck / UsageReport 的 Ready、Missing、ReadFailure、Unavailable 组合。
4. `desktop-client/src/ipc/models_tests.rs` 或对应集成测试
   追加带 `user_id` 与不带 `user_id` 的请求构造验证。
5. `desktop-client/src/approval_tests.rs` 或命令契约测试
   追加 ApprovalSubmit 对 `backend_user_id` 的依赖测试，确认不再 fallback 到 owner_id/JWT。
6. `admin-backend/tests/client_config_reports_*`
   继续保持 `backend_principal_id` 在 `/api/client-config` 契约中的存在性与兼容性测试。

## Priority

若只补最关键的最小集，优先实现下面 6 条：

1. Model 1 / Test 1
2. Model 1 / Test 5
3. Model 2 / Test 2
4. Model 2 / Test 7
5. Model 2 / Test 10
6. Model 2 / Test 17

这 6 条基本覆盖了本次语义拆分最容易回退或再次混用的风险点。