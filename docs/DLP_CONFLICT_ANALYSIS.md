# IronClaw 原有 DLP 能力与客户端 DLP 冲突分析

> 生成日期：2026-03-21
> 核心问题：IronClaw Safety 层的安全检测会不会和 Desktop-Client 自己的 DLP 模块冲突？

---

## 一、结论先行

**会冲突，但不严重，且可以优雅解决。**

| 冲突类型 | 严重程度 | 说明 |
|---------|---------|------|
| 凭证泄露检测重复扫描 | ⚠️ 中等 | 同一内容被扫描两次，浪费性能 |
| 拦截行为不一致 | 🔴 需要处理 | 客户端脱敏后，IronClaw 可能再次拦截或放行 |
| 审计事件重复 | ⚠️ 中等 | 同一事件可能产生两条审计记录 |
| 模式覆盖重叠 | ✅ 无害 | 重叠部分不会导致错误，只是冗余 |

---

## 二、两套 DLP 系统对比

### 数据流中的位置

```
用户输入
  │
  ▼
┌─────────────────────────────────┐
│ ① Desktop-Client DLP 扫描       │  ← 客户端 Tauri 层
│    scan_user_input()            │
│    - 管理端下发的正则/关键词规则  │
│    - 内置中国特色模式（身份证等） │
│    - LeakDetector（16 默认模式）  │
│    → 脱敏/阻止/放行              │
└──────────────┬──────────────────┘
               │ 脱敏后的内容
               ▼
┌─────────────────────────────────┐
│ ② IronClaw Gateway API          │  ← Sidecar 接收
│    POST /api/chat/send          │
└──────────────┬──────────────────┘
               │
               ▼
┌─────────────────────────────────┐
│ ③ IronClaw Safety 入站扫描      │  ← Agent 线程处理
│    scan_inbound_for_secrets()   │
│    - LeakDetector（16 默认模式） │
│    → 发现秘密则拒绝，返回警告    │
└──────────────┬──────────────────┘
               │ 通过检查
               ▼
┌─────────────────────────────────┐
│ ④ LLM 处理 → 工具调用           │
└──────────────┬──────────────────┘
               │ 工具输出
               ▼
┌─────────────────────────────────┐
│ ⑤ IronClaw Safety 出站扫描      │  ← 工具输出清理
│    sanitize_tool_output()       │
│    - LeakDetector 泄露检测       │
│    - Policy 策略检查（7 条规则） │
│    - Sanitizer 注入检测（18+4） │
│    → 脱敏/阻止/放行              │
└──────────────┬──────────────────┘
               │
               ▼
            返回给用户
```

### 各层检测能力对比

| 检测能力 | Desktop-Client DLP | IronClaw Safety |
|---------|-------------------|-----------------|
| **检测位置** | 用户输入前（Tauri 层） | Agent 处理中（入站+出站） |
| **凭证泄露检测** | ✅ LeakDetector 16 模式 + 中国特色模式 | ✅ LeakDetector 16 模式 |
| **中国身份证** | ✅ 18位/15位 | ❌ 无 |
| **中国手机号** | ✅ 含国际区号 | ❌ 无 |
| **中国银行卡** | ✅ | ❌ 无 |
| **阿里云/腾讯云 Key** | ✅ | ❌ 无 |
| **提示注入检测** | ❌ 无 | ✅ Sanitizer 18+4 模式 |
| **安全策略检查** | ❌ 无 | ✅ Policy 7 条规则 |
| **管理端动态规则** | ✅ 正则+关键词 | ❌ 硬编码（改造后支持） |
| **脱敏方式** | 替换为 `[REDACTED]` / 格式保留 | 拒绝发送 / 阻止输出 |
| **审计上报** | ✅ POST 到管理端 | ❌ 仅本地日志 |

---

## 三、具体冲突场景分析

### 场景 1：凭证泄露检测重复（⚠️ 中等）

**流程**：
```
用户输入: "我的 API key 是 sk-proj-abc123..."
  │
  ├─ ① Client DLP: LeakDetector 检测到 OpenAI key → 脱敏为 "sk-pr****"
  │   → 脱敏后内容发送给 IronClaw
  │
  └─ ③ IronClaw Safety: scan_inbound_for_secrets()
      → LeakDetector 再次扫描脱敏后的内容
      → "sk-pr****" 不匹配 OpenAI key 模式 → 放行 ✅
```

**结论**：不冲突。客户端先脱敏，IronClaw 看到的是脱敏后的内容，不会再次触发。但**扫描了两次**，有性能浪费。

### 场景 2：客户端脱敏不完整（🔴 需要处理）

**流程**：
```
用户输入: "Bearer eyJhbGciOiJIUzI1NiJ9.xxx..."
  │
  ├─ ① Client DLP: 管理端规则没有覆盖 Bearer token
  │   → 放行，原文发送给 IronClaw
  │
  └─ ③ IronClaw Safety: scan_inbound_for_secrets()
      → LeakDetector 检测到 Bearer token
      → 拒绝发送，返回警告 ⚠️
```

**问题**：用户看到 IronClaw 的英文警告 "Your message appears to contain a secret..."，而不是客户端的中文提示。**用户体验不一致**。

### 场景 3：客户端阻止 vs IronClaw 放行（✅ 无冲突）

**流程**：
```
用户输入: "我的身份证号是 330326199408015618"
  │
  ├─ ① Client DLP: 中国身份证模式匹配 → 脱敏为 "330***********5618"
  │   → 脱敏后内容发送给 IronClaw
  │
  └─ ③ IronClaw Safety: scan_inbound_for_secrets()
      → "330***********5618" 不匹配任何模式 → 放行 ✅
```

**结论**：不冲突。IronClaw 没有中国特色模式，客户端的脱敏结果不会被 IronClaw 误判。

### 场景 4：工具输出中的敏感信息（✅ 互补）

**流程**：
```
LLM 调用工具 → 工具返回包含 AWS Key 的内容
  │
  └─ ⑤ IronClaw Safety: sanitize_tool_output()
      → LeakDetector 检测到 AWS Key → 脱敏
      → Policy 检查通过
      → Sanitizer 注入检测通过
      → 返回脱敏后的内容给用户
```

**结论**：不冲突。工具输出只经过 IronClaw Safety，客户端 DLP 不参与。两者在不同环节各司其职。

### 场景 5：审计事件重复（⚠️ 中等）

**流程**：
```
用户输入包含 GitHub token
  │
  ├─ ① Client DLP: 检测到 → 脱敏 → 上报审计事件到管理端
  │
  └─ ③ IronClaw Safety: 脱敏后内容不触发 → 无审计事件
```

**但如果客户端 DLP 漏检**：
```
用户输入包含 Stripe key（客户端规则未覆盖）
  │
  ├─ ① Client DLP: 未检测到 → 原文发送
  │
  └─ ③ IronClaw Safety: 检测到 → 拒绝 → 仅本地日志，不上报管理端
```

**问题**：IronClaw 拦截的事件不会上报到管理端审计系统。改造后通过 AuditSink 解决。

---

## 四、冲突解决方案

### 方案：分层防御，各司其职

```
┌─────────────────────────────────────────────────┐
│              第一层：客户端 DLP（前置过滤）         │
│                                                  │
│  职责：                                           │
│  ├── 管理端下发的业务规则（身份证、手机号等）       │
│  ├── 中国特色敏感信息检测                          │
│  ├── 脱敏处理（格式保留、替换）                    │
│  └── 审计上报到管理端                              │
│                                                  │
│  特点：面向业务合规，规则可动态更新                  │
└──────────────────────┬──────────────────────────┘
                       │ 脱敏后的内容
                       ▼
┌─────────────────────────────────────────────────┐
│           第二层：IronClaw Safety（安全兜底）       │
│                                                  │
│  职责：                                           │
│  ├── 凭证泄露检测（API Key、Token 等）             │
│  ├── 提示注入防护（Prompt Injection）              │
│  ├── 安全策略执行（系统文件访问、Shell 注入等）     │
│  └── 工具输出清理                                  │
│                                                  │
│  特点：面向 AI 安全，硬编码兜底规则                  │
└─────────────────────────────────────────────────┘
```

### 具体措施

#### 1. 明确职责边界

| 检测类型 | 主要负责 | 兜底 |
|---------|---------|------|
| 业务敏感信息（身份证、手机号等） | 客户端 DLP | — |
| 管理端动态规则 | 客户端 DLP | — |
| API Key / Token 泄露 | 客户端 DLP | IronClaw LeakDetector |
| 提示注入防护 | — | IronClaw Sanitizer |
| 安全策略（系统文件、Shell 注入） | — | IronClaw Policy |
| 工具输出清理 | — | IronClaw Safety |

#### 2. 处理重复扫描

**短期（不改 IronClaw）**：接受重复扫描，性能影响可忽略。

LeakDetector 扫描一次约 ~50μs（参考 benchmark），重复扫描的性能开销极小。

**中期（改造后）**：可以在 IronClaw 配置中添加开关：

```toml
# 当作为 Sidecar 运行时，客户端已做前置 DLP 扫描
# 可以跳过入站秘密扫描，避免重复
[safety]
skip_inbound_secret_scan = true  # 客户端已处理
```

#### 3. 统一用户体验

**问题**：IronClaw 拦截时返回英文警告，客户端 DLP 返回中文提示。

**解决**：客户端拦截 IronClaw 的拒绝响应，替换为统一的中文提示：

```typescript
// 前端处理
try {
  const result = await invoke('send_chat_message', { content });
} catch (error) {
  if (error.includes('appears to contain a secret')) {
    // 替换为中文提示
    showToast('检测到敏感信息，消息已被拦截。请移除敏感内容后重试。');
  }
}
```

#### 4. 审计事件统一

改造后通过 AuditSink trait，IronClaw 的拦截事件也会上报到管理端：

```
客户端 DLP 拦截 → 直接上报管理端 ✅
IronClaw Safety 拦截 → AuditSink → 上报管理端 ✅（改造后）
```

---

## 五、重叠模式详细对比

### LeakDetector 模式（两边都有，完全相同）

| 模式名 | Desktop-Client | IronClaw | 重叠 |
|--------|:--------------:|:--------:|:----:|
| openai_api_key | ✅ | ✅ | 🔄 |
| anthropic_api_key | ✅ | ✅ | 🔄 |
| aws_access_key | ✅ | ✅ | 🔄 |
| github_token | ✅ | ✅ | 🔄 |
| github_fine_grained_pat | ✅ | ✅ | 🔄 |
| stripe_api_key | ✅ | ✅ | 🔄 |
| nearai_session | ✅ | ✅ | 🔄 |
| pem_private_key | ✅ | ✅ | 🔄 |
| ssh_private_key | ✅ | ✅ | 🔄 |
| google_api_key | ✅ | ✅ | 🔄 |
| slack_token | ✅ | ✅ | 🔄 |
| twilio_api_key | ✅ | ✅ | 🔄 |
| sendgrid_api_key | ✅ | ✅ | 🔄 |
| bearer_token | ✅ | ✅ | 🔄 |
| auth_header | ✅ | ✅ | 🔄 |
| high_entropy_hex | ✅ | ✅ | 🔄 |

**原因**：Desktop-Client 的 `DlpDetector::new()` 调用 `LeakDetector::new()`，继承了 IronClaw 的全部 16 个默认模式。

### Desktop-Client 独有模式

| 模式名 | 说明 |
|--------|------|
| chinese_id_card_18 | 中国 18 位身份证号 |
| chinese_id_card_15 | 中国 15 位身份证号 |
| chinese_mobile | 中国手机号 |
| chinese_mobile_intl | 中国手机号（含国际区号） |
| chinese_bank_card | 中国银行卡号 |
| aliyun_access_key | 阿里云 AccessKey |
| tencent_secret_id | 腾讯云 SecretId |
| 管理端动态规则 | 从 Admin-Backend 同步的自定义规则 |

### IronClaw 独有能力（客户端 DLP 没有）

| 能力 | 说明 |
|------|------|
| Sanitizer 注入检测 | 18 个 AhoCorasick + 4 个正则（提示注入防护） |
| Policy 策略检查 | 7 条安全规则（系统文件、Shell 注入等） |
| 工具输出清理 | sanitize_tool_output()（客户端 DLP 不参与） |
| 输出长度限制 | max_output_length 截断 |

---

## 六、总结

| 问题 | 答案 |
|------|------|
| 会冲突吗？ | 有重叠，但不会导致功能错误 |
| 重叠在哪？ | LeakDetector 的 16 个凭证检测模式完全相同 |
| 会双重拦截吗？ | 不会。客户端先脱敏，IronClaw 看到的是脱敏后内容 |
| 会漏检吗？ | 不会。两层防御，客户端漏检的 IronClaw 兜底 |
| 性能影响？ | 极小。LeakDetector 单次扫描 ~50μs |
| 需要改什么？ | 短期无需改动；中期可加 `skip_inbound_secret_scan` 开关 |

### 核心结论

> 两套 DLP 系统是**分层防御**关系，不是冲突关系。客户端 DLP 面向业务合规（身份证、手机号、管理端规则），IronClaw Safety 面向 AI 安全（注入防护、策略执行、工具输出清理）。16 个凭证检测模式的重叠是因为客户端 DLP 直接复用了 `ironclaw_safety` 的 `LeakDetector`，这是正确的设计——客户端做前置过滤，IronClaw 做安全兜底。
