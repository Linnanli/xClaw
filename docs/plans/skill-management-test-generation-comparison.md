# 技能管理测试生成对照

生成时间：2026-04-14

## 目的

这份文档选取“技能管理”作为具体模块，对照两种测试生成方式：

- 常规直觉式生成
- PICT 驱动生成

这里的“技能管理”指管理后台的技能上传、重扫、下架、注册表读取及相关扫描状态流转，主要参考 [admin-backend/src/handlers/extensions.rs](admin-backend/src/handlers/extensions.rs) 以及现有测试文件：

- [admin-backend/tests/extensions_unit_tests.rs](admin-backend/tests/extensions_unit_tests.rs)
- [admin-backend/tests/extensions_failure_tests.rs](admin-backend/tests/extensions_failure_tests.rs)
- [admin-backend/tests/extensions_contract_tests.rs](admin-backend/tests/extensions_contract_tests.rs)

本文档不是要替换现有测试，而是回答一个更具体的问题：如果同样让系统“生成一版测试”，PICT 驱动会比普通生成更好吗。

## 模块边界

本次对照覆盖以下入口：

- JSON 上传技能
- ZIP 包上传技能
- Rescan
- Yank
- 私有注册表读取

本次对照不覆盖：

- 插件管理
- 前端 UI 交互细节
- scanner 容器自身的独立行为

## 版本 A：常规直觉式生成

### 典型生成特征

如果不使用 PICT，而是让模型按常规方式为“技能管理”写一版测试，通常会得到下面这类结果：

- 优先覆盖 happy path
- 少量补充格式校验失败
- 对状态机、角色、扫描器状态、不同入口组合覆盖不足
- 很容易把 Rescan、Yank、RegistryRead 这些入口割裂开分别写

### 一版有代表性的常规测试清单

| # | 入口 | 场景 | 预期 |
| --- | --- | --- | --- |
| A-1 | JsonUpload | 最小合法 frontmatter | 上传成功 |
| A-2 | JsonUpload | 缺少 frontmatter | 返回 400 |
| A-3 | JsonUpload | name 非法 | 返回 400 |
| A-4 | JsonUpload | keywords 太短 | 返回 400 |
| A-5 | JsonUpload | 内容超过 64 KiB | 返回 400 |
| A-6 | PackageUpload | ZIP 中存在合法 SKILL.md | 上传成功 |
| A-7 | Rescan | skill 不存在 | 返回 404 |
| A-8 | Rescan | 文件内容为空 | 返回 400 |
| A-9 | Yank | 已经 yanked 的技能再次 yank | 返回 409 |
| A-10 | RegistryRead | approved 技能可以查询到 | 返回契约格式 |
| A-11 | RegistryRead | 非法 slug | 返回错误 |
| A-12 | ScanResult | scan_result 响应字段完整 | 满足契约 |

### 这一版的优点

- 易读，容易快速起步
- 适合校验函数和单个 API 行为
- 很贴合现有 [admin-backend/tests/extensions_unit_tests.rs](admin-backend/tests/extensions_unit_tests.rs) 的写法

### 这一版的缺点

- 容易漏掉角色和入口的交叉组合
- 容易漏掉“Payload 状态”和“Scanner 状态”的组合
- 很难系统性地证明：扫描超时、unsafe、缺版本、匿名访问这些维度有没有交叉覆盖到
- 对工作流类问题不够敏感，例如 upload -> scan -> pending 或 rescan 后 enabled/review_status 的联动

### 常规生成版示例代码骨架

```rust
#[test]
fn test_upload_accepts_minimal_valid_frontmatter() {
    let content = "---\nname: valid-skill\ndescription: x\n---\n# body";
    assert!(validate_skill_package(content).is_ok());
}

#[tokio::test]
async fn test_failure_rescan_skill_not_found() {
    let resp = post_json(app, "/api/skills/{id}/rescan", json!({})).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[test]
fn test_contract_scan_result_response_has_required_fields() {
    let response = json!({
        "skill_id": "...",
        "scan_result": {
            "scanner_type": "cisco-ai-skill-scanner",
            "verdict": "SAFE",
            "is_safe": true,
            "findings_count": 0,
            "findings": []
        }
    });
    assert!(response["scan_result"].get("verdict").is_some());
}
```

这类代码没有问题，但它覆盖的是“点”，不是“面”。

---

## 版本 B：PICT 驱动生成

### 设计思路

对“技能管理”更合理的做法，不是从函数一个个往外写，而是先抽出会影响行为的参数：

- EntryPoint
- AuthState
- PayloadState
- ScannerState
- AssertionLayer

然后用约束去排除无意义组合，再生成 pairwise 用例。

### PICT Model

```text
EntryPoint: JsonUpload, PackageUpload, Rescan, Yank, RegistryRead
AuthState: Anonymous, UserToken, AdminToken
PayloadState: Valid, MissingVersion, InvalidFrontmatter, MissingContent, UnsafeScan
ScannerState: Disabled, Safe, Unsafe, Timeout
AssertionLayer: Unit, Failure, Contract, Integration, SecurityAudit

IF [EntryPoint] IN {Rescan, Yank} THEN [PayloadState] IN {Valid, MissingContent, UnsafeScan};
IF [EntryPoint] = "RegistryRead" THEN [PayloadState] = "Valid";
IF [EntryPoint] = "Yank" THEN [ScannerState] = "Disabled";
IF [EntryPoint] = "RegistryRead" THEN [ScannerState] = "Disabled";
IF [PayloadState] = "UnsafeScan" THEN [ScannerState] IN {Unsafe, Timeout};
IF [PayloadState] = "MissingVersion" THEN [EntryPoint] IN {JsonUpload, PackageUpload};
IF [PayloadState] = "InvalidFrontmatter" THEN [EntryPoint] IN {JsonUpload, PackageUpload};
```

### 预期结果字典

| 代码 | 预期 |
| --- | --- |
| P1 | 上传成功，状态进入 scanning 或 pending，版本回退逻辑正确 |
| P2 | 校验失败并返回 400，错误信息明确 |
| P3 | 扫描结果 unsafe 或 timeout 时，不允许进入可发布状态 |
| P4 | 匿名或非授权访问被拒绝，不能产生越权副作用 |
| P5 | Rescan 能正确推进 review_status 和 enabled 联动 |
| P6 | RegistryRead 返回格式满足 ironclaw 消费契约 |
| P7 | 安全审计不泄露原始危险内容、敏感内容或内部扫描细节 |

### 生成出的 pairwise 用例

| # | EntryPoint | AuthState | PayloadState | ScannerState | AssertionLayer | Expected |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | JsonUpload | Anonymous | Valid | Disabled | Unit | P4 |
| 2 | JsonUpload | UserToken | MissingVersion | Safe | Failure | P1 |
| 3 | JsonUpload | AdminToken | InvalidFrontmatter | Unsafe | Contract | P2 |
| 4 | PackageUpload | Anonymous | MissingVersion | Unsafe | Integration | P1 |
| 5 | PackageUpload | UserToken | Valid | Timeout | Contract | P3 |
| 6 | PackageUpload | AdminToken | MissingContent | Disabled | Failure | P2 |
| 7 | Rescan | Anonymous | MissingContent | Safe | Contract | P4 |
| 8 | Rescan | UserToken | UnsafeScan | Unsafe | Unit | P3 |
| 9 | JsonUpload | Anonymous | UnsafeScan | Timeout | SecurityAudit | P7 |
| 10 | Rescan | AdminToken | Valid | Safe | Integration | P5 |
| 11 | Yank | UserToken | MissingContent | Disabled | Integration | P4 |
| 12 | PackageUpload | UserToken | InvalidFrontmatter | Safe | SecurityAudit | P2 |
| 13 | PackageUpload | AdminToken | MissingVersion | Timeout | Unit | P1 |
| 14 | RegistryRead | AdminToken | Valid | Disabled | SecurityAudit | P6 |
| 15 | JsonUpload | Anonymous | InvalidFrontmatter | Timeout | Failure | P2 |
| 16 | JsonUpload | Anonymous | MissingContent | Unsafe | SecurityAudit | P7 |
| 17 | JsonUpload | AdminToken | UnsafeScan | Timeout | Integration | P3 |
| 18 | Rescan | Anonymous | Valid | Unsafe | Failure | P4 |
| 19 | Yank | Anonymous | Valid | Disabled | Contract | P4 |
| 20 | Rescan | Anonymous | MissingContent | Timeout | Unit | P4 |
| 21 | JsonUpload | Anonymous | MissingVersion | Disabled | Contract | P1 |
| 22 | JsonUpload | Anonymous | InvalidFrontmatter | Disabled | Unit | P2 |
| 23 | PackageUpload | Anonymous | UnsafeScan | Unsafe | Failure | P3 |
| 24 | Rescan | Anonymous | Valid | Disabled | SecurityAudit | P4 |
| 25 | Yank | AdminToken | Valid | Disabled | Unit | P5 |
| 26 | RegistryRead | Anonymous | Valid | Disabled | Unit | P4 |
| 27 | RegistryRead | UserToken | Valid | Disabled | Failure | P4 |
| 28 | JsonUpload | Anonymous | Valid | Safe | Unit | P4 |
| 29 | JsonUpload | Anonymous | MissingVersion | Disabled | SecurityAudit | P1 |
| 30 | JsonUpload | Anonymous | InvalidFrontmatter | Disabled | Integration | P2 |
| 31 | JsonUpload | Anonymous | UnsafeScan | Unsafe | Contract | P3 |
| 32 | Yank | Anonymous | Valid | Disabled | Failure | P4 |
| 33 | Yank | Anonymous | Valid | Disabled | SecurityAudit | P4 |
| 34 | RegistryRead | Anonymous | Valid | Disabled | Contract | P4 |
| 35 | RegistryRead | Anonymous | Valid | Disabled | Integration | P4 |

### PICT 版的优点

- 能系统覆盖入口、角色、载荷状态、扫描器状态、测试层级这 5 个维度
- 能更快暴露“某一维已经测了，但和其他维从未交叉”的盲区
- 很适合技能管理这种状态机和工作流重的模块
- 更容易把生成结果映射到现有 unit、failure、contract、integration、security audit 测试族

### PICT 版的缺点

- 如果直接按表硬写，测试数量会上升
- 有些组合在业务上价值不高，需要人工裁剪
- 不适合替代所有小而直接的校验函数测试

### PICT 驱动版代码骨架

```rust
#[tokio::test]
async fn test_skill_management_pairwise_json_upload_missing_version_timeout() {
    // EntryPoint=JsonUpload
    // AuthState=AdminToken
    // PayloadState=MissingVersion
    // ScannerState=Timeout
    // AssertionLayer=Unit
    // 预期：版本默认回退，不应进入可发布状态
}

#[tokio::test]
async fn test_skill_management_pairwise_rescan_valid_safe_admin() {
    // EntryPoint=Rescan
    // AuthState=AdminToken
    // PayloadState=Valid
    // ScannerState=Safe
    // AssertionLayer=Integration
    // 预期：review_status / enabled 联动正确
}

#[tokio::test]
async fn test_skill_management_pairwise_registry_read_anonymous_contract() {
    // EntryPoint=RegistryRead
    // AuthState=Anonymous
    // PayloadState=Valid
    // ScannerState=Disabled
    // AssertionLayer=Contract
    // 预期：匿名访问是否被拒绝，或返回格式是否符合预期
}
```

---

## 对照结论

### 哪个“效果更好”

对技能管理这个模块，PICT 驱动生成整体上更好，但不是所有层面都更好。

### 更好的地方

| 维度 | 常规生成 | PICT 驱动 |
| --- | --- | --- |
| Happy path 覆盖 | 好 | 好 |
| 参数组合覆盖 | 一般 | 很好 |
| 角色 × 状态 × 入口交叉覆盖 | 弱 | 强 |
| 失败路径系统性 | 一般 | 好 |
| 安全审计组合覆盖 | 弱 | 好 |
| 写出来的代码可读性 | 好 | 一般 |
| 人工裁剪成本 | 低 | 中 |

### 实际建议

对技能管理模块，不要二选一。

更合理的组合是：

1. 用常规方式保留小而直接的校验函数测试。
2. 用 PICT 方式覆盖 upload、rescan、yank、registry 这些工作流入口。
3. 把 PICT 生成结果映射到现有测试文件，而不是新建一套平行的“大而全测试”。

### 在这个仓库里的落地策略

- 小型格式校验继续放在 [admin-backend/tests/extensions_unit_tests.rs](admin-backend/tests/extensions_unit_tests.rs)
- 失败路径组合优先放在 [admin-backend/tests/extensions_failure_tests.rs](admin-backend/tests/extensions_failure_tests.rs)
- 响应结构和注册表契约继续放在 [admin-backend/tests/extensions_contract_tests.rs](admin-backend/tests/extensions_contract_tests.rs)
- 扫描 unsafe、timeout、敏感内容泄露组合优先放在现有 security audit 与 integration 测试族

## 最终结论

如果目标只是“快速补几条测试”，常规生成更省事。

如果目标是“更系统地补技能管理这类工作流模块的测试盲区”，PICT 驱动生成明显更强，尤其是在以下问题上：

- 角色和权限交叉覆盖
- 扫描状态和上传入口交叉覆盖
- rescan / yank / registry 这些状态迁移路径覆盖
- 安全审计与失败路径组合覆盖

所以对这个模块来说，PICT 不是替代原有生成方式，而是把原来的“按感觉补测试”升级成“按组合矩阵补测试”。
